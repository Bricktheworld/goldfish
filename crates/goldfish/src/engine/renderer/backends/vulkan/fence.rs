use super::device::VulkanDevice;
use ash::vk;
use tracy_client as tracy;

#[derive(Clone)]
pub struct VulkanFence {
	pub raw: vk::Fence,
}

impl VulkanDevice {
	pub fn create_fence(&self, signaled: bool) -> VulkanFence {
		tracy::span!();
		unsafe {
			let create_info = vk::FenceCreateInfo::builder().flags(if signaled {
				vk::FenceCreateFlags::SIGNALED
			} else {
				vk::FenceCreateFlags::default()
			});

			let raw = self
				.raw
				.create_fence(&create_info, None)
				.expect("Failed to create VulkanFence");

			VulkanFence { raw }
		}
	}

	pub fn destroy_fence(&self, fence: VulkanFence) {
		tracy::span!();
		unsafe {
			self.raw.destroy_fence(fence.raw, None);
		}
	}
}

impl VulkanFence {
	pub fn wait(&self, device: &VulkanDevice) {
		tracy::span!();
		unsafe {
			device
				.raw
				.wait_for_fences(&[self.raw], true, std::u64::MAX)
				.expect("Failed to wait for VulkanFence!");
		}
	}

	pub fn wait_multiple(device: &VulkanDevice, fences: &[&VulkanFence], wait_all: bool) {
		tracy::span!();
		unsafe {
			if fences.is_empty() {
				return;
			}

			let vk_fences: Vec<vk::Fence> = fences.iter().map(|f| f.raw).collect();

			device
				.raw
				.wait_for_fences(&vk_fences, wait_all, std::u64::MAX)
				.expect("Failed to wait for VulkanFences!");
		}
	}

	pub fn reset(&self, device: &VulkanDevice) {
		tracy::span!();
		unsafe {
			device
				.raw
				.reset_fences(&[self.raw])
				.expect("Failed to reset VulkanFence");
		}
	}
}
