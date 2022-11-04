use super::{device::VulkanDevice, VulkanDeviceChild};
use ash::vk;

pub struct VulkanSemaphore
{
	semaphore: vk::Semaphore,
	destroyed: bool,
}

impl VulkanSemaphore
{
	pub fn new(device: &VulkanDevice) -> Self
	{
		unsafe {
			let semaphore = device
				.vk_device()
				.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
				.expect("Failed to create VulkanSemaphore");
			Self {
				semaphore,
				destroyed: false,
			}
		}
	}

	pub fn get(&self) -> vk::Semaphore
	{
		self.semaphore
	}
}

impl VulkanDeviceChild for VulkanSemaphore
{
	fn destroy(mut self, device: &VulkanDevice)
	{
		unsafe {
			device.vk_device().destroy_semaphore(self.semaphore, None);
		}
		self.destroyed = true;
	}
}

impl Drop for VulkanSemaphore
{
	fn drop(&mut self)
	{
		assert!(
			self.destroyed,
			"destroy(&VulkanDevice) was not called before VulkanSemaphore was dropped!"
		);
	}
}
