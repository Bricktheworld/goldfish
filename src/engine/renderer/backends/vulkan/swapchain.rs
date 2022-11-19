use super::{
	command_pool::{QueueType, VulkanCommandBuffer, VulkanCommandPool},
	device::{VulkanDevice, VulkanDeviceChild},
	fence::VulkanFence,
	semaphore::VulkanSemaphore,
	SwapchainError,
};

use crate::types::Size;

use ash::{extensions::khr::Swapchain, vk};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{
	atomic::{AtomicU32, Ordering},
	Arc, RwLock,
};

pub struct VulkanSwapchain
{
	pub device: VulkanDevice,

	image_format: vk::Format,
	extent: vk::Extent2D,
	render_pass: vk::RenderPass,
	swapchain_loader: Swapchain,
	swapchain: vk::SwapchainKHR,

	images: Vec<SwapchainImage>,

	frames: Vec<VulkanFrame>,
	frame_resource_manager: VulkanFrameResourceManager,
	current_frame: Arc<AtomicU32>,
}

#[derive(Clone)]
pub struct VulkanFrameResourceManager
{
	in_use_resources: Vec<Arc<RwLock<HashSet<u64>>>>,
	fences: Vec<Rc<VulkanFence>>,
	current_frame: Arc<AtomicU32>,
}

impl VulkanFrameResourceManager
{
	pub fn new(current_frame: &Arc<AtomicU32>, fences: Vec<Rc<VulkanFence>>) -> Self
	{
		Self {
			in_use_resources: vec![Arc::new(RwLock::new(HashSet::new())); fences.len()],
			current_frame: Arc::clone(current_frame),
			fences,
		}
	}

	pub fn use_resource<T>(&mut self, resource: T)
	where
		T: vk::Handle,
	{
		self.in_use_resources[self.current_frame.load(Ordering::SeqCst) as usize]
			.write()
			.unwrap()
			.insert(resource.as_raw());
	}

	pub fn is_using_resource<T>(&self, resource: T) -> bool
	where
		T: vk::Handle,
	{
		self.in_use_resources[self.current_frame.load(Ordering::SeqCst) as usize]
			.read()
			.unwrap()
			.contains(&resource.as_raw())
	}

	pub fn clear_resources(&mut self)
	{
		self.in_use_resources[self.current_frame.load(Ordering::SeqCst) as usize]
			.write()
			.unwrap()
			.clear();
	}

	pub fn wait_resource_not_in_use<T>(&self, device: &VulkanDevice, resource: T)
	where
		T: vk::Handle,
	{
		let resource = &resource.as_raw();

		let fences: Vec<&VulkanFence> = self
			.in_use_resources
			.iter()
			.enumerate()
			.flat_map(|(i, resources)| {
				let resources = resources.read().unwrap();
				if resources.contains(&resource)
				{
					Some(&self.fences[i] as &VulkanFence)
				}
				else
				{
					None
				}
			})
			.collect();

		VulkanFence::wait_multiple(device, &fences, true);
	}
}

impl VulkanSwapchain
{
	const MAX_FRAMES_IN_FLIGHT: usize = 2;

	pub fn new(framebuffer_size: Size, device: VulkanDevice) -> Self
	{
		let (image_format, extent, swapchain_loader, swapchain, render_pass, images) =
			Self::init_swapchain(framebuffer_size, &device);
		let mut frames = Vec::with_capacity(Self::MAX_FRAMES_IN_FLIGHT);

		for _ in 0..Self::MAX_FRAMES_IN_FLIGHT
		{
			frames.push(VulkanFrame {
				command_pool: VulkanCommandPool::new(&device, QueueType::GRAPHICS),
				completed_fence: Rc::new(VulkanFence::new(&device, true)),
				acquired_sem: VulkanSemaphore::new(&device),
				present_sem: VulkanSemaphore::new(&device),
			});
		}

		let current_frame = Arc::new(AtomicU32::new(0));
		let frame_resource_manager = VulkanFrameResourceManager::new(
			&current_frame,
			frames
				.iter()
				.map(|frame| Rc::clone(&frame.completed_fence))
				.collect(),
		);

		Self {
			device,
			image_format,
			extent,
			render_pass,
			swapchain_loader,
			swapchain,

			images,

			frames,
			frame_resource_manager,
			current_frame,
		}
	}

	pub fn acquire(&mut self) -> Result<FrameInfo, SwapchainError>
	{
		assert!(
			self.current_frame.load(Ordering::SeqCst) < Self::MAX_FRAMES_IN_FLIGHT as u32,
			"Invalid swapchain current frame!"
		);

		// Get the current frame that we are processing
		let current_frame = self.current_frame.load(Ordering::SeqCst) as usize;
		let frame = &mut self.frames[current_frame];

		// Wait for the frame to have fully finished rendering before acquiring.
		frame.completed_fence.wait(&self.device);

		// Clear frame resources
		self.frame_resource_manager.clear_resources();

		match unsafe {
			self.swapchain_loader.acquire_next_image(
				self.swapchain,
				std::u64::MAX,
				frame.acquired_sem.get(),
				vk::Fence::null(),
			)
		}
		{
			Ok((image_index, false)) =>
			{
				assert!(
					image_index < self.images.len() as u32,
					"Invalid image index received!"
				);

				let image = &mut self.images[image_index as usize];

				if let Some(ref fence) = image.available_fence
				{
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
	) -> Result<(), SwapchainError>
	{
		let current_frame = self.current_frame.load(Ordering::SeqCst) as usize;

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
						.wait_semaphores(&[acquired_sem.get()])
						.wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
						.command_buffers(&[command_buffer])
						.signal_semaphores(&[present_sem.get()])
						.build()],
					frame.completed_fence.get(),
				)
				.unwrap();
		}

		self.current_frame.store(
			((current_frame + 1) % Self::MAX_FRAMES_IN_FLIGHT) as u32,
			Ordering::SeqCst,
		);

		let present_queue = self.device.present_queue.lock().unwrap();
		match unsafe {
			self.swapchain_loader.queue_present(
				*present_queue,
				&vk::PresentInfoKHR::builder()
					.wait_semaphores(&[present_sem.get()])
					.swapchains(&[self.swapchain])
					.image_indices(&[image_index]),
			)
		}
		{
			Ok(suboptimal) =>
			{
				if suboptimal
				{
					return Err(SwapchainError::SubmitSuboptimal);
				}
				else
				{
					return Ok(());
				}
			}
			Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) =>
			{
				Err(SwapchainError::SubmitSuboptimal)
			}
			Err(_) =>
			{
				panic!("Failed to present swapchain images!");
			}
		}
	}

	pub fn invalidate(&mut self, framebuffer_size: Size)
	{
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

	fn destroy_swapchain(&mut self)
	{
		unsafe {
			for image in std::mem::take(&mut self.images).into_iter()
			{
				image.destroy(&self.device);
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
	)
	{
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
			.find(|&mode| mode == vk::PresentModeKHR::IMMEDIATE)
			.unwrap_or(vk::PresentModeKHR::FIFO);

		let extent = if capabilities.current_extent.width != std::u32::MAX
		{
			capabilities.current_extent
		}
		else
		{
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
		if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count
		{
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

		if queue_indices.graphics_family != queue_indices.present_family
		{
			create_info = create_info
				.image_sharing_mode(vk::SharingMode::CONCURRENT)
				.queue_family_indices(&queue_family_indices);
		}
		else
		{
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
					image,
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

	pub fn get_frame(&self, index: usize) -> &VulkanFrame
	{
		&self.frames[index]
	}

	pub fn get_frame_mut(&mut self, index: usize) -> &mut VulkanFrame
	{
		&mut self.frames[index]
	}

	pub fn raw_device(&self) -> &ash::Device
	{
		&self.device.raw
	}

	pub fn extent(&self) -> vk::Extent2D
	{
		self.extent
	}

	pub fn render_pass(&self) -> vk::RenderPass
	{
		self.render_pass
	}

	pub fn destroy(&mut self)
	{
		self.device.wait_idle();

		self.destroy_swapchain();

		for frame in std::mem::take(&mut self.frames).into_iter()
		{
			frame.destroy(&self.device);
		}

		for fence in std::mem::take(&mut self.frame_resource_manager.fences).into_iter()
		{
			if let Ok(fence) = Rc::try_unwrap(fence)
			{
				fence.destroy(&self.device);
			}
		}
	}
}

struct SwapchainImage
{
	image: vk::Image,
	image_view: vk::ImageView,
	framebuffer: vk::Framebuffer,

	available_fence: Option<Rc<VulkanFence>>,
}

impl VulkanDeviceChild for SwapchainImage
{
	fn destroy(self, device: &VulkanDevice)
	{
		unsafe {
			let vk_device = &device.raw;

			vk_device.destroy_framebuffer(self.framebuffer, None);
			vk_device.destroy_image_view(self.image_view, None);
		}
	}
}

pub struct VulkanFrame
{
	command_pool: VulkanCommandPool,
	completed_fence: Rc<VulkanFence>,
	acquired_sem: VulkanSemaphore,
	present_sem: VulkanSemaphore,
}

impl VulkanDeviceChild for VulkanFrame
{
	fn destroy(self, device: &VulkanDevice)
	{
		self.command_pool.destroy(device);

		if let Ok(completed_fence) = Rc::try_unwrap(self.completed_fence)
		{
			completed_fence.destroy(device);
		}

		self.acquired_sem.destroy(device);
		self.present_sem.destroy(device);
	}
}

pub struct FrameInfo
{
	pub output_framebuffer: vk::Framebuffer,
	pub image_index: u32,
	pub frame_index: usize,
	pub command_buffer: VulkanCommandBuffer,
}
