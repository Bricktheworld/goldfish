pub mod vulkan;

pub enum Backend
{
	Vulkan(vulkan::device::VulkanGraphicsDevice),
}
