mod command_pool;
mod device;
mod fence;
mod framebuffer;
mod semaphore;
mod swapchain;
mod texture;

use crate::window::Window;
use command_pool::VulkanCommandBuffer;
use device::VulkanDevice;
use swapchain::{FrameInfo, VulkanSwapchain};

use crate::types::{Color, Size};
use ash::vk;
use custom_error::custom_error;
use std::sync::RwLockReadGuard;

custom_error! {pub SwapchainError
	SubmitSuboptimal = "Swapchain is suboptimal and needs to be recreated",
	AcquireSuboptimal = "Swapchain is suboptimal and needs to be recreated"
}

#[derive(Clone)]
pub struct VulkanGraphicsDevice
{
	device: VulkanDevice,
}

impl VulkanGraphicsDevice
{
	pub fn new(window: &Window) -> (Self, VulkanGraphicsContext)
	{
		let device = VulkanDevice::new(window);
		let swapchain = VulkanSwapchain::new(window.get_size(), device.clone());

		(
			Self { device },
			VulkanGraphicsContext {
				swapchain,
				current_frame_info: None,
				output_framebuffer_is_bound: true,
			},
		)
	}

	pub fn wait_idle(&self)
	{
		self.device.wait_idle();
	}

	pub fn destroy(&mut self)
	{
		self.device.destroy();
	}
}

pub struct VulkanGraphicsContext
{
	swapchain: VulkanSwapchain,
	current_frame_info: Option<FrameInfo>,
	output_framebuffer_is_bound: bool,
}

impl VulkanGraphicsContext
{
	pub fn begin_frame(&mut self, window: &Window) -> Result<(), SwapchainError>
	{
		assert!(
			self.current_frame_info.is_none(),
			"Did not call end_frame before starting another frame!"
		);

		match self.swapchain.acquire()
		{
			Ok(res) =>
			{
				self.current_frame_info = Some(res);

				Ok(())
			}
			Err(err) =>
			{
				self.swapchain.invalidate(window.get_size());
				Err(err)
			}
		}
	}

	pub fn end_frame(&mut self, window: &Window)
	{
		if let Some(current_frame_info) = self.current_frame_info.take()
		{
			// {
			// 	let frame = self.swapchain.get_frame_mut(current_frame_info.frame_index);

			// 	frame.end_command_buffer(&self.swapchain.device, current_frame_info.command_buffer);
			// }

			if let Err(_) = self.swapchain.submit(
				current_frame_info.image_index,
				current_frame_info.command_buffer,
			)
			{
				self.swapchain.invalidate(window.get_size());
			}
		}
		else
		{
			panic!("Did not call begin_frame first!");
		}
	}

	pub fn bind_output_framebuffer(&mut self, color: Color)
	{
		let cmd = self.get_command_buffer();

		unsafe {
			self.raw_device().cmd_set_viewport(
				cmd,
				0,
				&[vk::Viewport::builder()
					.x(0.0)
					.y(self.swapchain.extent().height as f32)
					.width(self.swapchain.extent().width as f32)
					.height(-(self.swapchain.extent().height as f32))
					.min_depth(0.0)
					.max_depth(1.0)
					.build()],
			);

			self.raw_device().cmd_set_scissor(
				cmd,
				0,
				&[vk::Rect2D::builder()
					.offset(vk::Offset2D { x: 0, y: 0 })
					.extent(self.swapchain.extent())
					.build()],
			);

			self.raw_device().cmd_begin_render_pass(
				cmd,
				&vk::RenderPassBeginInfo::builder()
					.render_pass(self.swapchain.render_pass())
					.framebuffer(self.get_output_framebuffer())
					.render_area(vk::Rect2D {
						offset: vk::Offset2D { x: 0, y: 0 },
						extent: self.swapchain.extent(),
					})
					.clear_values(&[vk::ClearValue {
						color: vk::ClearColorValue {
							float32: [color.r, color.g, color.b, color.a],
						},
					}]),
				vk::SubpassContents::INLINE,
			);
		}

		self.output_framebuffer_is_bound = true;
	}

	pub fn unbind_output_framebuffer(&mut self)
	{
		assert!(self.output_framebuffer_is_bound, "Unbinding output framebuffer not allowed without first binding with `bind_output_framebuffer`");
		unsafe {
			self.raw_device()
				.cmd_end_render_pass(self.get_command_buffer());
		}
		self.output_framebuffer_is_bound = false;
	}

	fn get_command_buffer(&self) -> VulkanCommandBuffer
	{
		self.current_frame_info
			.as_ref()
			.expect("begin_frame was not called!")
			.command_buffer
	}

	fn get_output_framebuffer(&self) -> vk::Framebuffer
	{
		self.current_frame_info
			.as_ref()
			.expect("begin_frame was not called!")
			.output_framebuffer
	}

	pub fn raw_device(&self) -> &ash::Device
	{
		self.swapchain.raw_device()
	}

	pub fn on_resize(&mut self, framebuffer_size: Size)
	{
		self.swapchain.invalidate(framebuffer_size);
	}

	pub fn destroy(&mut self)
	{
		self.swapchain.destroy();
	}
}
