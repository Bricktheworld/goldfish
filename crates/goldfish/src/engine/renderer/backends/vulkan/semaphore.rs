use super::device::VulkanDevice;
use ash::vk;
use tracy_client as tracy;

pub struct VulkanSemaphore {
	pub raw: vk::Semaphore,
}

impl VulkanDevice {
	pub fn create_semaphore(&self) -> VulkanSemaphore {
		tracy::span!();
		unsafe {
			let raw = self
				.raw
				.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
				.expect("Failed to create VulkanSemaphore");
			VulkanSemaphore { raw }
		}
	}

	pub fn destroy_semaphore(&self, semaphore: VulkanSemaphore) {
		tracy::span!();
		unsafe {
			self.raw.destroy_semaphore(semaphore.raw, None);
		}
	}
}
