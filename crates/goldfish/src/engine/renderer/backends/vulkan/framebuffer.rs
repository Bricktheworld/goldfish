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
	pub attachments: Vec<VulkanTexture>,
}

impl VulkanDevice {
	pub fn create_framebuffer(
		&self,
		width: u32,
		height: u32,
		render_pass: &VulkanRenderPass,
	) -> VulkanFramebuffer {
		let attachments = render_pass
			.color_attachments
			.iter()
			.map(|attachment| {
				self.create_texture(
					width,
					height,
					attachment.format,
					attachment.usage | TextureUsage::ATTACHMENT,
				)
			})
			.chain(render_pass.depth_attachment.as_ref().map(|attachment| {
				self.create_texture(
					width,
					height,
					attachment.format,
					attachment.usage | TextureUsage::ATTACHMENT,
				)
			}))
			.collect::<Vec<_>>();

		let image_views = attachments
			.iter()
			.map(|texture| texture.image_view)
			.collect::<Vec<_>>();

		let raw = unsafe {
			self.raw
				.create_framebuffer(
					&vk::FramebufferCreateInfo::builder()
						.render_pass(render_pass.raw)
						.attachments(&image_views)
						.width(width)
						.height(height)
						.layers(1u32),
					None,
				)
				.expect("Failed to create framebuffer!")
		};

		VulkanFramebuffer {
			width,
			height,
			raw,
			attachments,
		}
	}

	pub fn destroy_framebuffer(&mut self, framebuffer: VulkanFramebuffer) {
		for attachment in framebuffer.attachments {
			self.destroy_texture(attachment);
		}
		self.queue_destruction(&mut [VulkanDestructor::Framebuffer(framebuffer.raw)]);
	}
}
