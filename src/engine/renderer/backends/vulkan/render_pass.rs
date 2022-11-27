use super::device::VulkanDevice;
use crate::renderer::{AttachmentDescription, LoadOp, StoreOp};
use ash::vk;

pub struct VulkanRenderPass
{
	pub raw: vk::RenderPass,
	pub color_attachments: Vec<AttachmentDescription>,
	pub depth_attachment: Option<AttachmentDescription>,
}

impl AttachmentDescription
{
	fn to_vk(
		&self,
		device: &VulkanDevice,
		initial_layout: vk::ImageLayout,
		final_layout: vk::ImageLayout,
	) -> vk::AttachmentDescription
	{
		vk::AttachmentDescription {
			format: self.format.to_vk(device),
			samples: vk::SampleCountFlags::TYPE_1,
			load_op: match self.load_op
			{
				LoadOp::Load => vk::AttachmentLoadOp::LOAD,
				LoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
				LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
			},
			store_op: match self.store_op
			{
				StoreOp::Store => vk::AttachmentStoreOp::STORE,
				StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
			},
			initial_layout,
			final_layout,
			..Default::default()
		}
	}
}

impl VulkanDevice
{
	pub fn create_render_pass(
		&self,
		color_attachments: &[AttachmentDescription],
		depth_attachment: Option<AttachmentDescription>,
	) -> VulkanRenderPass
	{
		let render_pass_attachments = color_attachments
			.iter()
			.map(|desc| {
				desc.to_vk(
					self,
					vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
					vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
				)
			})
			.chain(depth_attachment.as_ref().map(|desc| {
				desc.to_vk(
					self,
					vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
					vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
				)
			}))
			.collect::<Vec<_>>();

		let color_attachment_refs = (0..color_attachments.len() as u32)
			.map(|attachment| vk::AttachmentReference {
				attachment,
				layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			})
			.collect::<Vec<_>>();

		let depth_attachment_ref = vk::AttachmentReference {
			attachment: color_attachments.len() as u32,
			layout: vk::ImageLayout::DEPTH_ATTACHMENT_STENCIL_READ_ONLY_OPTIMAL,
		};

		let mut subpass_description = vk::SubpassDescription::builder()
			.color_attachments(&color_attachment_refs)
			.pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);
		if depth_attachment.is_some()
		{
			subpass_description =
				subpass_description.depth_stencil_attachment(&depth_attachment_ref);
		}

		let subpass_description = subpass_description.build();

		let subpasses = [subpass_description];
		let render_pass_info = vk::RenderPassCreateInfo::builder()
			.attachments(&render_pass_attachments)
			.subpasses(&subpasses);

		let raw = unsafe {
			self.raw
				.create_render_pass(&render_pass_info, None)
				.expect("Failed to create render pass!")
		};

		VulkanRenderPass {
			raw,
			color_attachments: color_attachments.to_vec(),
			depth_attachment,
		}
	}

	pub fn destroy_render_pass(&self, render_pass: VulkanRenderPass)
	{
		unsafe {
			self.raw.destroy_render_pass(render_pass.raw, None);
		}
	}
}
