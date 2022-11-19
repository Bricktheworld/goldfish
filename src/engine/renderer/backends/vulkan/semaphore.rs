use super::device::{VulkanDevice, VulkanDeviceChild};
use ash::vk;

pub struct VulkanSemaphore
{
	semaphore: vk::Semaphore,
}

impl VulkanSemaphore
{
	pub fn new(device: &VulkanDevice) -> Self
	{
		unsafe {
			let semaphore = device
				.raw
				.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
				.expect("Failed to create VulkanSemaphore");
			Self { semaphore }
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
			device.raw.destroy_semaphore(self.semaphore, None);
		}
	}
}
