use super::command_pool::VulkanCommandPool;
use super::device::VulkanDevice;
use super::fence::VulkanFence;
use super::semaphore::VulkanSemaphore;

use crate::types::Size;

use ash::{extensions::khr::Swapchain, vk};
use std::rc::Rc;
use std::sync::{Arc, RwLock, Weak};

pub struct VulkanSwapchain
{
	device: Arc<VulkanDevice>,

	image_format: vk::Format,
	extent: vk::Extent2D,
	render_pass: vk::RenderPass,
	swapchain_loader: Swapchain,
	swapchain: vk::SwapchainKHR,

	images: Vec<SwapchainImage>,
}

impl VulkanSwapchain
{
	pub fn new(framebuffer_size: Size, device: VulkanDevice) -> Self
	{
		let device = Arc::new(device);

		let swapchain_details = device.get_swapchain_details();

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

		let pick_swap_extent = || -> vk::Extent2D {
			if capabilities.current_extent.width != std::u32::MAX
			{
				return capabilities.current_extent;
			}
			else
			{
				return vk::Extent2D {
					width: framebuffer_size.width.clamp(
						capabilities.min_image_extent.width,
						capabilities.max_image_extent.width,
					),
					height: framebuffer_size.height.clamp(
						capabilities.min_image_extent.height,
						capabilities.max_image_extent.height,
					),
				};
			}
		};
		let extent = pick_swap_extent();

		let mut image_count = capabilities.min_image_count + 1;
		if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count
		{
			image_count = capabilities.max_image_count;
		}

		let swapchain_loader = Swapchain::new(device.vk_instance(), device.vk_device());
		let mut create_info = vk::SwapchainCreateInfoKHR::builder()
			.surface(device.vk_surface())
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
				.vk_device()
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
					.vk_device()
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
					.vk_device()
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
					device: Arc::downgrade(&device),
				}
			})
			.collect();

		Self {
			device,
			image_format,
			extent,
			swapchain_loader,
			swapchain,
			render_pass,

			images,
		}
	}
}

impl Drop for VulkanSwapchain
{
	fn drop(&mut self)
	{
		unsafe {
			self.images.clear();

			self.device
				.vk_device()
				.destroy_render_pass(self.render_pass, None);

			self.swapchain_loader
				.destroy_swapchain(self.swapchain, None);
		}
	}
}

struct SwapchainImage
{
	image: vk::Image,
	image_view: vk::ImageView,
	framebuffer: vk::Framebuffer,

	available_fence: Option<Rc<VulkanFence>>,

	device: Weak<VulkanDevice>,
}

impl Drop for SwapchainImage
{
	fn drop(&mut self)
	{
		unsafe {
			let device = self.device.upgrade().unwrap();

			let vk_device = device.vk_device();

			vk_device.destroy_framebuffer(self.framebuffer, None);
			vk_device.destroy_image_view(self.image_view, None);
			vk_device.destroy_image(self.image, None);
		}
	}
}

pub struct VulkanFrame
{
	command_pool: VulkanCommandPool,
	completed_fence: VulkanFence,
	acquired_sem: VulkanSemaphore,
	present_sem: VulkanSemaphore,
}
