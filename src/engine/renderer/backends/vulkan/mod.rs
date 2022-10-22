mod command_buffer;
mod command_pool;
mod device;
mod fence;
mod semaphore;
mod swapchain;

use crate::window::Window;
use device::VulkanDevice;
use swapchain::VulkanSwapchain;

use std::sync::{Arc, RwLock};

pub struct VulkanGraphicsDevice
{
	swapchain: Arc<RwLock<VulkanSwapchain>>,
}

impl VulkanGraphicsDevice
{
	pub fn new(window: &Window) -> Self
	{
		let device = VulkanDevice::new(window);
		let swapchain = VulkanSwapchain::new(window.get_size(), device);
		Self {
			swapchain: Arc::new(RwLock::new(swapchain)),
		}
	}
}
