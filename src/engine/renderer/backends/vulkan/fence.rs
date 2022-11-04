use super::{device::VulkanDevice, VulkanDeviceChild};
use ash::vk;

pub struct VulkanFence
{
	fence: vk::Fence,
	destroyed: bool,
}

impl VulkanFence
{
	pub fn new(device: &VulkanDevice, signaled: bool) -> Self
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
				.vk_device()
				.create_fence(&create_info, None)
				.expect("Failed to create VulkanFence");

			Self {
				fence,
				destroyed: false,
			}
		}
	}

	pub fn get(&self) -> vk::Fence
	{
		self.fence
	}

	pub fn wait(&self, device: &VulkanDevice)
	{
		unsafe {
			device
				.vk_device()
				.wait_for_fences(&[self.fence], true, std::u64::MAX)
				.expect("Failed to wait for VulkanFence!");
		}
	}

	pub fn wait_multiple(device: &VulkanDevice, fences: &[&VulkanFence], wait_all: bool)
	{
		unsafe {
			if fences.is_empty()
			{
				return;
			}

			let vk_fences: Vec<vk::Fence> = fences.iter().map(|f| f.fence).collect();

			device
				.vk_device()
				.wait_for_fences(&vk_fences, wait_all, std::u64::MAX)
				.expect("Failed to wait for VulkanFences!");
		}
	}

	pub fn reset(&self, device: &VulkanDevice)
	{
		unsafe {
			device
				.vk_device()
				.reset_fences(&[self.fence])
				.expect("Failed to reset VulkanFence");
		}
	}
}

impl VulkanDeviceChild for VulkanFence
{
	fn destroy(mut self, device: &VulkanDevice)
	{
		unsafe {
			device.vk_device().destroy_fence(self.fence, None);
		}
		self.destroyed = true;
	}
}

impl Drop for VulkanFence
{
	fn drop(&mut self)
	{
		assert!(
			self.destroyed,
			"destroy(&VulkanDevice) was not called before VulkanFence was dropped!"
		);
	}
}
