use super::device::VulkanDevice;
use ash::vk;

pub struct VulkanFramebuffer
{
	pub width: u32,
	pub height: u32,
	pub raw: vk::Framebuffer,
}

impl VulkanDevice
{
	pub fn create_framebuffer(&self) -> VulkanFramebuffer
	{
		todo!()
	}

	pub fn destroy_framebuffer(&self, framebuffer: VulkanFramebuffer)
	{
		unsafe {
			self.raw.destroy_framebuffer(framebuffer.raw, None);
		}
	}
}
