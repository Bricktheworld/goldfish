use super::{
	device::{VulkanDestructor, VulkanDevice},
	render_pass::VulkanRenderPass,
	texture::VulkanTexture,
};
use crate::renderer::TextureUsage;
use ash::vk;

pub struct VulkanFramebuffer {
	pub width: u32,
	pub height: u32,
	pub raw: vk::Framebuffer,
}

impl VulkanDevice {
	pub fn create_framebuffer(
		&self,
		width: u32,
		height: u32,
		render_pass: &VulkanRenderPass,
		attachments: &[&VulkanTexture],
	) -> VulkanFramebuffer {
		let attachments = attachments.iter().map(|a| a.image_view).collect::<Vec<_>>();

		let raw = unsafe {
			self.raw
				.create_framebuffer(
					&vk::FramebufferCreateInfo::builder()
						.render_pass(render_pass.raw)
						.attachments(&attachments)
						.width(width)
						.height(height)
						.layers(1u32),
					None,
				)
				.expect("Failed to create framebuffer!")
		};

		VulkanFramebuffer { width, height, raw }
	}

	pub fn destroy_framebuffer(&mut self, framebuffer: VulkanFramebuffer) {
		self.queue_destruction(&mut [VulkanDestructor::Framebuffer(framebuffer.raw)]);
	}
}
