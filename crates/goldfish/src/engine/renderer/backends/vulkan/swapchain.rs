use super::{
	command_pool::{QueueType, VulkanCommandBuffer, VulkanCommandPool},
	device::VulkanDevice,
	fence::VulkanFence,
	pipeline::VulkanPipeline,
	semaphore::VulkanSemaphore,
	SwapchainError,
};

use crate::types::Size;

use ash::{extensions::khr::Swapchain, vk};
use std::rc::Rc;
use tracy_client as tracy;

pub struct VulkanSwapchain {
	pub device: VulkanDevice,

	pub image_format: vk::Format,
	pub extent: vk::Extent2D,
	pub render_pass: vk::RenderPass,
	pub swapchain_loader: Swapchain,
	pub swapchain: vk::SwapchainKHR,

	images: Vec<SwapchainImage>,

	pub frames: Vec<VulkanFrame>,

	pub pipelines: Vec<Option<VulkanPipeline>>,
}

impl VulkanSwapchain {
	pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

	pub fn new(framebuffer_size: Size, device: VulkanDevice) -> Self {
		let (image_format, extent, swapchain_loader, swapchain, render_pass, images) =
			Self::init_swapchain(framebuffer_size, &device);
		let mut frames = Vec::with_capacity(Self::MAX_FRAMES_IN_FLIGHT);

		for _ in 0..Self::MAX_FRAMES_IN_FLIGHT {
			frames.push(VulkanFrame {
				command_pool: device.create_command_pool(QueueType::GRAPHICS),
				completed_fence: Rc::new(device.create_fence(true)),
				acquired_sem: device.create_semaphore(),
				present_sem: device.create_semaphore(),
			});
		}

		Self {
			device,
			image_format,
			extent,
			render_pass,
			swapchain_loader,
			swapchain,

			images,

			frames,
			pipelines: Default::default(),
		}
	}

	pub fn acquire(&mut self) -> Result<FrameInfo, SwapchainError> {
		let mut guard = self.device.frame.lock().unwrap();
		let current_frame = guard.frame as usize;
		assert!(
			current_frame < Self::MAX_FRAMES_IN_FLIGHT,
			"Invalid swapchain current frame!"
		);
		tracy::span!();

		// Get the current frame that we are processing
		let frame = &self.frames[current_frame];

		// Wait for the frame to have fully finished rendering before acquiring.
		frame.completed_fence.wait(&self.device);

		let destructors = std::mem::take(&mut guard.destructors[current_frame]);
		for destructor in destructors.into_iter() {
			self.device.run_destructor(destructor);
		}

		guard.destructors[current_frame].clear();

		match unsafe {
			self.swapchain_loader.acquire_next_image(
				self.swapchain,
				u64::MAX,
				frame.acquired_sem.raw,
				vk::Fence::null(),
			)
		} {
			Ok((image_index, false)) => {
				assert!(
					image_index < self.images.len() as u32,
					"Invalid image index received!"
				);

				let image = &mut self.images[image_index as usize];

				if let Some(ref fence) = image.available_fence {
					fence.wait(&self.device);
				}

				image.available_fence = Some(Rc::clone(&frame.completed_fence));

				self.frames[current_frame]
					.command_pool
					.recycle(&self.device);
				let command_buffer = self.frames[current_frame]
					.command_pool
					.begin_command_buffer(&self.device);

				Ok(FrameInfo {
					image_index,
					frame_index: current_frame,
					output_framebuffer: image.framebuffer,
					command_buffer,
				})
			}
			Ok((_, true)) => Err(SwapchainError::AcquireSuboptimal),
			Err(_) => Err(SwapchainError::AcquireSuboptimal),
		}
	}

	pub fn submit(
		&mut self,
		image_index: u32,
		command_buffer: VulkanCommandBuffer,
	) -> Result<(), SwapchainError> {
		tracy::span!();
		let mut guard = self.device.frame.lock().unwrap();
		let current_frame = guard.frame as usize;

		self.frames[current_frame]
			.command_pool
			.end_command_buffer(&self.device, command_buffer);
		let frame = &self.frames[current_frame];

		let acquired_sem = &frame.acquired_sem;
		let present_sem = &frame.present_sem;

		unsafe {
			frame.completed_fence.reset(&self.device);

			let graphics_queue = self.device.graphics_queue.lock().unwrap();
			self.device
				.raw
				.queue_submit(
					*graphics_queue,
					&[vk::SubmitInfo::builder()
						.wait_semaphores(&[acquired_sem.raw])
						.wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
						.command_buffers(&[command_buffer])
						.signal_semaphores(&[present_sem.raw])
						.build()],
					frame.completed_fence.raw,
				)
				.unwrap();
		}

		guard.frame = ((current_frame + 1) % Self::MAX_FRAMES_IN_FLIGHT) as u32;

		let present_queue = self.device.present_queue.lock().unwrap();
		match unsafe {
			self.swapchain_loader.queue_present(
				*present_queue,
				&vk::PresentInfoKHR::builder()
					.wait_semaphores(&[present_sem.raw])
					.swapchains(&[self.swapchain])
					.image_indices(&[image_index]),
			)
		} {
			Ok(suboptimal) => {
				return if suboptimal {
					Err(SwapchainError::SubmitSuboptimal)
				} else {
					Ok(())
				};
			}
			Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
				Err(SwapchainError::SubmitSuboptimal)
			}
			Err(_) => {
				panic!("Failed to present swapchain images!");
			}
		}
	}

	pub fn invalidate(&mut self, framebuffer_size: Size) {
		tracy::span!();
		self.device.wait_idle();

		self.destroy_swapchain();

		let (image_format, extent, swapchain_loader, swapchain, render_pass, images) =
			Self::init_swapchain(framebuffer_size, &self.device);

		self.image_format = image_format;
		self.extent = extent;
		self.swapchain_loader = swapchain_loader;
		self.swapchain = swapchain;
		self.render_pass = render_pass;
		self.images = images;
	}

	fn destroy_swapchain(&mut self) {
		tracy::span!();
		unsafe {
			for image in std::mem::take(&mut self.images).into_iter() {
				self.device.raw.destroy_framebuffer(image.framebuffer, None);
				self.device.raw.destroy_image_view(image.image_view, None);
			}

			self.device.raw.destroy_render_pass(self.render_pass, None);

			self.swapchain_loader
				.destroy_swapchain(self.swapchain, None);
		}
	}

	fn init_swapchain(
		framebuffer_size: Size,
		device: &VulkanDevice,
	) -> (
		vk::Format,
		vk::Extent2D,
		Swapchain,
		vk::SwapchainKHR,
		vk::RenderPass,
		Vec<SwapchainImage>,
	) {
		tracy::span!();
		let swapchain_details = device.query_swapchain_details();

		let capabilities = &swapchain_details.capabilities;

		let surface_format = swapchain_details
			.surface_formats
			.iter()
			.find(|&format| {
				(format.format == vk::Format::R8G8B8A8_UNORM
					|| format.format == vk::Format::B8G8R8A8_UNORM)
					&& format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
			})
			.expect("No surface formats found!");

		let present_mode = swapchain_details
			.present_modes
			.iter()
			.cloned()
			.find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
			.unwrap_or(vk::PresentModeKHR::FIFO);

		let extent = if capabilities.current_extent.width != u32::MAX {
			capabilities.current_extent
		} else {
			vk::Extent2D {
				width: framebuffer_size.width.clamp(
					capabilities.min_image_extent.width,
					capabilities.max_image_extent.width,
				),
				height: framebuffer_size.height.clamp(
					capabilities.min_image_extent.height,
					capabilities.max_image_extent.height,
				),
			}
		};

		let mut image_count = capabilities.min_image_count + 1;
		if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
			image_count = capabilities.max_image_count;
		}

		let swapchain_loader = Swapchain::new(&device.instance, &device.raw);
		let mut create_info = vk::SwapchainCreateInfoKHR::builder()
			.surface(device.surface)
			.min_image_count(image_count)
			.image_format(surface_format.format)
			.image_color_space(surface_format.color_space)
			.image_extent(extent)
			.image_array_layers(1)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

		let queue_indices = device.get_queue_family_indices();
		let queue_family_indices = [queue_indices.graphics_family, queue_indices.present_family];

		if queue_indices.graphics_family != queue_indices.present_family {
			create_info = create_info
				.image_sharing_mode(vk::SharingMode::CONCURRENT)
				.queue_family_indices(&queue_family_indices);
		} else {
			create_info = create_info
				.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
				.queue_family_indices(&[]);
		}

		create_info = create_info
			.pre_transform(capabilities.current_transform)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			.present_mode(present_mode)
			.clipped(true);

		let swapchain = unsafe {
			swapchain_loader
				.create_swapchain(&create_info, None)
				.expect("Failed to create swapchain")
		};

		let image_format = surface_format.format;

		let render_pass = unsafe {
			device
				.raw
				.create_render_pass(
					&vk::RenderPassCreateInfo::builder()
						.attachments(&[vk::AttachmentDescription::builder()
							.format(image_format)
							.samples(vk::SampleCountFlags::TYPE_1)
							.load_op(vk::AttachmentLoadOp::CLEAR)
							.store_op(vk::AttachmentStoreOp::STORE)
							.stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
							.stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
							.initial_layout(vk::ImageLayout::UNDEFINED)
							.final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
							.build()])
						.subpasses(&[vk::SubpassDescription::builder()
							.pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
							.color_attachments(&[vk::AttachmentReference::builder()
								.attachment(0)
								.layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
								.build()])
							.build()])
						.dependencies(&[vk::SubpassDependency::builder()
							.src_subpass(vk::SUBPASS_EXTERNAL)
							.dst_subpass(0)
							.src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
							.dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
							.src_access_mask(vk::AccessFlags::default())
							.dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
							.build()]),
					None,
				)
				.expect("Failed to create Render Pass!")
		};

		let vk_images = unsafe {
			swapchain_loader
				.get_swapchain_images(swapchain)
				.expect("Failed to get swapchain images")
		};

		let images: Vec<SwapchainImage> = vk_images
			.into_iter()
			.map(|image| unsafe {
				let image_view = device
					.raw
					.create_image_view(
						&vk::ImageViewCreateInfo::builder()
							.image(image)
							.view_type(vk::ImageViewType::TYPE_2D)
							.format(image_format)
							.components(
								vk::ComponentMapping::builder()
									.r(vk::ComponentSwizzle::IDENTITY)
									.g(vk::ComponentSwizzle::IDENTITY)
									.b(vk::ComponentSwizzle::IDENTITY)
									.a(vk::ComponentSwizzle::IDENTITY)
									.build(),
							)
							.subresource_range(
								vk::ImageSubresourceRange::builder()
									.aspect_mask(vk::ImageAspectFlags::COLOR)
									.base_mip_level(0)
									.level_count(1)
									.base_array_layer(0)
									.layer_count(1)
									.build(),
							),
						None,
					)
					.expect("Failed to create image view!");

				let framebuffer = device
					.raw
					.create_framebuffer(
						&vk::FramebufferCreateInfo::builder()
							.render_pass(render_pass)
							.attachments(&[image_view])
							.width(extent.width)
							.height(extent.height)
							.layers(1),
						None,
					)
					.expect("Failed to create framebuffer!");

				SwapchainImage {
					image_view,
					framebuffer,
					available_fence: None,
				}
			})
			.collect();

		(
			image_format,
			extent,
			swapchain_loader,
			swapchain,
			render_pass,
			images,
		)
	}

	pub fn raw_device(&self) -> &ash::Device {
		&self.device.raw
	}

	pub fn destroy(&mut self) {
		tracy::span!();
		self.device.wait_idle();

		self.destroy_swapchain();

		for frame in std::mem::take(&mut self.frames).into_iter() {
			self.device.destroy_command_pool(frame.command_pool);

			if let Ok(completed_fence) = Rc::try_unwrap(frame.completed_fence) {
				self.device.destroy_fence(completed_fence);
			}

			self.device.destroy_semaphore(frame.acquired_sem);
			self.device.destroy_semaphore(frame.present_sem);
		}
	}
}

struct SwapchainImage {
	image_view: vk::ImageView,
	framebuffer: vk::Framebuffer,

	available_fence: Option<Rc<VulkanFence>>,
}

pub struct VulkanFrame {
	command_pool: VulkanCommandPool,
	completed_fence: Rc<VulkanFence>,
	acquired_sem: VulkanSemaphore,
	present_sem: VulkanSemaphore,
}

pub struct FrameInfo {
	pub output_framebuffer: vk::Framebuffer,
	pub image_index: u32,
	pub frame_index: usize,
	pub command_buffer: VulkanCommandBuffer,
}
