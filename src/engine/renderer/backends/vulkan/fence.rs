use super::device::VulkanDevice;
use ash::vk;
use std::sync::Weak;

pub struct VulkanFence
{
	device: Weak<VulkanDevice>,
	fence: vk::Fence,
}

impl VulkanFence
{
	pub fn new(device: Weak<VulkanDevice>, signaled: bool) -> Self
	{
		unsafe {
			let create_info = vk::FenceCreateInfo::builder().flags(
				if signaled
				{
					vk::FenceCreateFlags::SIGNALED
				}
				else
				{
					vk::FenceCreateFlags::default()
				},
			);

			let fence = device
				.upgrade()
				.unwrap()
				.vk_device()
				.create_fence(&create_info, None)
				.expect("Failed to create VulkanFence");

			Self { device, fence }
		}
	}

	pub fn get(&self) -> &vk::Fence
	{
		&self.fence
	}

	pub fn wait(&self)
	{
		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.wait_for_fences(&[self.fence], true, std::u64::MAX)
				.expect("Failed to wait for VulkanFence!");
		}
	}

	pub fn wait_multiple(fences: &[VulkanFence], wait_all: bool)
	{
		unsafe {
			if fences.is_empty()
			{
				return;
			}

			let vk_fences: Vec<vk::Fence> = fences.iter().map(|f| f.fence).collect();

			let device = &fences.first().unwrap().device;
			device
				.upgrade()
				.unwrap()
				.vk_device()
				.wait_for_fences(&vk_fences, wait_all, std::u64::MAX)
				.expect("Failed to wait for VulkanFences!");
		}
	}

	pub fn reset(&self)
	{
		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.reset_fences(&[self.fence])
				.expect("Failed to reset VulkanFence");
		}
	}
}

impl Drop for VulkanFence
{
	fn drop(&mut self)
	{
		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.destroy_fence(self.fence, None);
		}
	}
}
