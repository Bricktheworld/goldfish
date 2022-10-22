use super::device::VulkanDevice;
use ash::vk;
use std::sync::Weak;

pub struct VulkanSemaphore
{
	device: Weak<VulkanDevice>,
	semaphore: vk::Semaphore,
}

impl VulkanSemaphore
{
	pub fn new(device: Weak<VulkanDevice>) -> Self
	{
		unsafe {
			let semaphore = device
				.upgrade()
				.unwrap()
				.vk_device()
				.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
				.expect("Failed to create VulkanSemaphore");
			Self { device, semaphore }
		}
	}

	pub fn get(&self) -> vk::Semaphore
	{
		self.semaphore
	}
}

impl Drop for VulkanSemaphore
{
	fn drop(&mut self)
	{
		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.destroy_semaphore(self.semaphore, None);
		}
	}
}
