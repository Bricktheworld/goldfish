mod command_pool;
mod device;
mod fence;
mod semaphore;
mod swapchain;

use crate::window::Window;
use command_pool::VulkanCommandBuffer;
use device::VulkanDevice;
use swapchain::VulkanSwapchain;

use ash::vk;
use custom_error::custom_error;
use std::sync::Arc;

custom_error! {pub SwapchainError
	SubmitSuboptimal = "Swapchain is suboptimal and needs to be recreated",
	AcquireSuboptimal = "Swapchain is suboptimal and needs to be recreated"
}

#[derive(Clone)]
pub struct VulkanGraphicsDevice
{
	device: Arc<VulkanDevice>,
}

impl VulkanGraphicsDevice
{
	pub fn new(window: &Window) -> (Self, VulkanGraphicsContext)
	{
		let device = Arc::new(VulkanDevice::new(window));
		let swapchain = VulkanSwapchain::new(window.get_size(), Arc::clone(&device));

		(
			Self { device },
			VulkanGraphicsContext {
				swapchain,
				current_frame_info: None,
			},
		)
	}
}

pub struct VulkanGraphicsContext
{
	swapchain: VulkanSwapchain,
	current_frame_info: Option<CurrentFrameInfo>,
}

struct CurrentFrameInfo
{
	image_index: u32,
	frame_index: usize,
	command_buffer: VulkanCommandBuffer,
	output_framebuffer: vk::Framebuffer,
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
				let frame = self.swapchain.get_frame_mut(res.frame_index);
				frame.recycle_command_pool();

				let command_buffer = frame.begin_command_buffer();
				self.current_frame_info = Some(CurrentFrameInfo {
					image_index: res.image_index,
					frame_index: res.frame_index,
					command_buffer,
					output_framebuffer: res.output_framebuffer,
				});

				Ok(())
			}
			Err(err) =>
			{
				self.swapchain.invalidate(window.get_size());
				Err(err)
			}
		}
	}

	pub fn end_frame(&mut self)
	{
		if let Some(current_frame_info) = self.current_frame_info.take()
		{
			let frame = self.swapchain.get_frame_mut(current_frame_info.frame_index);

			frame.end_command_buffer(current_frame_info.command_buffer);

			self.swapchain.submit(
				current_frame_info.image_index,
				&[current_frame_info.command_buffer],
			);
		}
		else
		{
			panic!("Did not call begin_frame first!");
		}
	}
}
