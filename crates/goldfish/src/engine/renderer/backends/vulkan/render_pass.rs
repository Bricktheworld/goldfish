use super::{
	device::{VulkanDestructor, VulkanDevice},
	pipeline::VulkanPipeline,
};
use crate::renderer::{AttachmentDescription, ImageLayout, LoadOp, StoreOp};
use ash::vk;

pub struct VulkanRenderPass {
	pub raw: vk::RenderPass,
	pub color_attachments: Vec<AttachmentDescription>,
	pub depth_attachment: Option<AttachmentDescription>,
}

impl From<LoadOp> for vk::AttachmentLoadOp {
	fn from(load_op: LoadOp) -> Self {
		match load_op {
			LoadOp::Load => vk::AttachmentLoadOp::LOAD,
			LoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
			LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
		}
	}
}

impl From<StoreOp> for vk::AttachmentStoreOp {
	fn from(store_op: StoreOp) -> Self {
		match store_op {
			StoreOp::Store => vk::AttachmentStoreOp::STORE,
			StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
		}
	}
}

impl From<ImageLayout> for vk::ImageLayout {
	fn from(image_layout: ImageLayout) -> Self {
		match image_layout {
			ImageLayout::Undefined => vk::ImageLayout::UNDEFINED,
			ImageLayout::General => vk::ImageLayout::GENERAL,
			ImageLayout::ColorAttachmentOptimal => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			ImageLayout::DepthStencilAttachmentOptimal => {
				vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
			}
			ImageLayout::DepthStencilReadOnlyOptimal => {
				vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
			}
			ImageLayout::ShaderReadOnlyOptimal => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
			ImageLayout::TransferSrcOptimal => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
			ImageLayout::TransferDstOptimal => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
			ImageLayout::Preinitialized => vk::ImageLayout::PREINITIALIZED,
		}
	}
}

impl AttachmentDescription {
	fn to_vk(&self, device: &VulkanDevice) -> vk::AttachmentDescription {
		vk::AttachmentDescription {
			format: self.format.to_vk(device),
			samples: vk::SampleCountFlags::TYPE_1,
			load_op: self.load_op.into(),
			store_op: self.store_op.into(),
			initial_layout: self.initial_layout.into(),
			final_layout: self.final_layout.into(),
			..Default::default()
		}
	}
}

impl VulkanDevice {
	pub fn create_render_pass(
		&self,
		color_attachments: &[AttachmentDescription],
		depth_attachment: Option<AttachmentDescription>,
	) -> VulkanRenderPass {
		let render_pass_attachments = color_attachments
			.iter()
			.map(|desc| desc.to_vk(self))
			.chain(depth_attachment.as_ref().map(|desc| desc.to_vk(self)))
			.collect::<Vec<_>>();

		let color_attachment_refs = (0..color_attachments.len() as u32)
			.map(|attachment| vk::AttachmentReference {
				attachment,
				layout: color_attachments[attachment as usize].final_layout.into(),
			})
			.collect::<Vec<_>>();

		let depth_attachment_ref = vk::AttachmentReference {
			attachment: color_attachments.len() as u32,
			layout: if let Some(depth_attachment) = depth_attachment {
				depth_attachment.final_layout.into()
			} else {
				ImageLayout::DepthStencilReadOnlyOptimal.into()
			},
		};

		let mut subpass_description = vk::SubpassDescription::builder()
			.color_attachments(&color_attachment_refs)
			.pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);
		if depth_attachment.is_some() {
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

	pub fn destroy_render_pass(&mut self, render_pass: VulkanRenderPass) {
		self.queue_destruction(&mut [VulkanDestructor::RenderPass(render_pass.raw)]);
	}
}
