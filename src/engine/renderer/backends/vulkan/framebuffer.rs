use ash::vk;

pub struct VulkanFramebuffer
{
	width: u32,
	height: u32,
	render_pass: vk::RenderPass,
	framebuffer: vk::Framebuffer,
}
