use super::{
	VulkanGraphicsContext, VulkanRasterCmd,
	{
		descriptor::VulkanDescriptorLayout,
		device::{VulkanDestructor, VulkanDevice},
		render_pass::VulkanRenderPass,
		shader::VulkanShader,
		swapchain::VulkanSwapchain,
	},
};
use crate::renderer::{DepthCompareOp, FaceCullMode, PolygonMode, Vertex, VertexAttributeDescriptionBinding, VertexAttributeFormat, VertexInputInfo, CS_MAIN, PS_MAIN, VS_MAIN};
use ash::vk;
use std::collections::{hash_map::Entry, HashMap};
use std::ffi::CString;

use tracy_client as tracy;

pub struct VulkanPipeline {
	pub pipeline: vk::Pipeline,
	pub pipeline_layout: vk::PipelineLayout,
}

type DescriptorSetLayout = HashMap<u32, rspirv_reflect::DescriptorInfo>;
type StageDescriptorSetLayouts = HashMap<u32, DescriptorSetLayout>;

impl From<FaceCullMode> for vk::CullModeFlags {
	fn from(m: FaceCullMode) -> Self {
		match m {
			FaceCullMode::Front => vk::CullModeFlags::FRONT,
			FaceCullMode::Back => vk::CullModeFlags::BACK,
			FaceCullMode::FrontAndBack => vk::CullModeFlags::FRONT_AND_BACK,
			FaceCullMode::NoCull => vk::CullModeFlags::NONE,
		}
	}
}

impl From<PolygonMode> for vk::PolygonMode {
	fn from(m: PolygonMode) -> Self {
		match m {
			PolygonMode::Fill => vk::PolygonMode::FILL,
			PolygonMode::Line => vk::PolygonMode::LINE,
			PolygonMode::Point => vk::PolygonMode::POINT,
		}
	}
}

impl From<VertexAttributeFormat> for vk::Format {
	fn from(f: VertexAttributeFormat) -> Self {
		match f {
			VertexAttributeFormat::F32 => Self::R32_SFLOAT,
			VertexAttributeFormat::F32Vec2 => Self::R32G32_SFLOAT,
			VertexAttributeFormat::F32Vec3 => Self::R32G32B32_SFLOAT,
			VertexAttributeFormat::F32Vec4 => Self::R32G32B32A32_SFLOAT,
		}
	}
}

impl From<VertexAttributeDescriptionBinding> for vk::VertexInputAttributeDescription {
	fn from(d: VertexAttributeDescriptionBinding) -> Self {
		Self {
			binding: 0,
			location: d.location,
			format: d.format.into(),
			offset: d.offset,
		}
	}
}

impl From<DepthCompareOp> for vk::CompareOp {
	fn from(o: DepthCompareOp) -> Self {
		match o {
			DepthCompareOp::Never => vk::CompareOp::NEVER,
			DepthCompareOp::Less => vk::CompareOp::LESS,
			DepthCompareOp::Equal => vk::CompareOp::EQUAL,
			DepthCompareOp::LessOrEqual => vk::CompareOp::LESS_OR_EQUAL,
			DepthCompareOp::Greater => vk::CompareOp::GREATER,
			DepthCompareOp::GreaterOrEqual => vk::CompareOp::GREATER_OR_EQUAL,
			DepthCompareOp::NotEqual => vk::CompareOp::NOT_EQUAL,
			DepthCompareOp::Always => vk::CompareOp::ALWAYS,
		}
	}
}

impl VulkanDevice {
	pub fn create_raster_pipeline(
		&self,
		vs: &VulkanShader,
		ps: Option<&VulkanShader>,
		descriptor_layouts: &[VulkanDescriptorLayout],
		render_pass: &VulkanRenderPass,
		depth_compare_op: Option<DepthCompareOp>,
		depth_write: bool,
		face_cull: FaceCullMode,
		push_constant_bytes: usize,
		vertex_input_info: VertexInputInfo,
		polygon_mode: PolygonMode,
	) -> VulkanPipeline {
		self.create_raster_pipeline_impl(
			vs,
			ps,
			descriptor_layouts,
			render_pass.raw,
			render_pass.color_attachments.len(),
			depth_compare_op,
			depth_write,
			face_cull,
			push_constant_bytes,
			vertex_input_info,
			polygon_mode,
		)
	}

	pub fn create_raster_pipeline_impl(
		&self,
		vs: &VulkanShader,
		ps: Option<&VulkanShader>,
		descriptor_layouts: &[VulkanDescriptorLayout],
		render_pass: vk::RenderPass,
		color_attachments_count: usize,
		depth_compare_op: Option<DepthCompareOp>,
		depth_write: bool,
		face_cull: FaceCullMode,
		push_constant_bytes: usize,
		vertex_input_info: VertexInputInfo,
		polygon_mode: PolygonMode,
	) -> VulkanPipeline {
		let mut layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(descriptor_layouts);

		let push_constant_range = vk::PushConstantRange {
			stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS,
			offset: 0,
			size: push_constant_bytes as u32,
		};

		if push_constant_bytes > 0 {
			layout_create_info = layout_create_info.push_constant_ranges(std::slice::from_ref(&push_constant_range));
		}

		let pipeline_layout = unsafe { self.raw.create_pipeline_layout(&layout_create_info, None).expect("Failed to create pipeline layout!") };

		let entry_names = [CString::new(VS_MAIN).unwrap(), CString::new(PS_MAIN).unwrap()];
		let mut shader_stage_infos = vec![vk::PipelineShaderStageCreateInfo::builder()
			.module(vs.module)
			.stage(vk::ShaderStageFlags::VERTEX)
			.name(&entry_names[0])
			.build()];

		if let Some(ps) = ps {
			shader_stage_infos.push(
				vk::PipelineShaderStageCreateInfo::builder()
					.module(ps.module)
					.stage(vk::ShaderStageFlags::FRAGMENT)
					.name(&entry_names[1])
					.build(),
			);
		}

		let binding_descriptions = [vk::VertexInputBindingDescription::builder().binding(0).stride(vertex_input_info.stride).build()];
		let attribute_descriptions = vertex_input_info.bindings.iter().map(|&b| b.into()).collect::<Vec<_>>();

		let vertex_input_state_info = if !vertex_input_info.bindings.is_empty() {
			vk::PipelineVertexInputStateCreateInfo::builder()
				.vertex_binding_descriptions(&binding_descriptions)
				.vertex_attribute_descriptions(&attribute_descriptions)
		} else {
			vk::PipelineVertexInputStateCreateInfo::builder()
		};

		let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder().topology(vk::PrimitiveTopology::TRIANGLE_LIST).build();

		let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder().viewport_count(1).scissor_count(1).build();

		let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
			front_face: vk::FrontFace::COUNTER_CLOCKWISE,
			line_width: 1.0,
			polygon_mode: polygon_mode.into(),
			cull_mode: face_cull.into(),
			..Default::default()
		};

		let multisample_state_info = vk::PipelineMultisampleStateCreateInfo {
			rasterization_samples: vk::SampleCountFlags::TYPE_1,
			..Default::default()
		};

		let noop_stencil_state = vk::StencilOpState {
			fail_op: vk::StencilOp::KEEP,
			pass_op: vk::StencilOp::KEEP,
			depth_fail_op: vk::StencilOp::KEEP,
			compare_op: vk::CompareOp::ALWAYS,
			..Default::default()
		};

		let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
			depth_test_enable: if depth_compare_op.is_some() { 1 } else { 0 },
			depth_write_enable: if depth_write { 1 } else { 0 },
			depth_compare_op: depth_compare_op.map_or(vk::CompareOp::default(), |c| c.into()),
			depth_bounds_test_enable: 0,
			stencil_test_enable: 0,
			// front: noop_stencil_state,
			// back: noop_stencil_state,
			..Default::default()
		};

		let color_blend_attachment_states = vec![
			vk::PipelineColorBlendAttachmentState {
				blend_enable: 0,
				src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
				dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
				color_blend_op: vk::BlendOp::ADD,
				src_alpha_blend_factor: vk::BlendFactor::ZERO,
				dst_alpha_blend_factor: vk::BlendFactor::ZERO,
				alpha_blend_op: vk::BlendOp::ADD,
				color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
			};
			color_attachments_count
		];

		let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder().attachments(&color_blend_attachment_states);

		let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
		let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

		let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
			.stages(&shader_stage_infos)
			.vertex_input_state(&vertex_input_state_info)
			.input_assembly_state(&vertex_input_assembly_state_info)
			.viewport_state(&viewport_state_info)
			.rasterization_state(&rasterization_info)
			.multisample_state(&multisample_state_info)
			.depth_stencil_state(&depth_state_info)
			.color_blend_state(&color_blend_state)
			.dynamic_state(&dynamic_state_info)
			.layout(pipeline_layout)
			.render_pass(render_pass);

		let pipeline = unsafe {
			self.raw
				.create_graphics_pipelines(vk::PipelineCache::null(), &[graphics_pipeline_info.build()], None)
				.expect("Failed to create graphics pipeline!")
		}[0];

		VulkanPipeline { pipeline, pipeline_layout }
	}

	pub fn create_compute_pipeline(&self, cs: &VulkanShader, descriptor_layouts: &[VulkanDescriptorLayout]) -> VulkanPipeline {
		let layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(descriptor_layouts);

		let pipeline_layout = unsafe { self.raw.create_pipeline_layout(&layout_create_info, None).expect("Failed to create pipeline layout!") };

		let name = CString::new(CS_MAIN).unwrap();
		let stage = vk::PipelineShaderStageCreateInfo::builder().module(cs.module).stage(vk::ShaderStageFlags::COMPUTE).name(&name);

		let compute_pipeline_info = vk::ComputePipelineCreateInfo::builder().layout(pipeline_layout).stage(stage.build());
		let pipeline = unsafe {
			self.raw
				.create_compute_pipelines(vk::PipelineCache::null(), &[compute_pipeline_info.build()], None)
				.expect("Failed to create compute pipeline!")
		}[0];
		VulkanPipeline { pipeline, pipeline_layout }
	}

	pub fn destroy_pipeline(&mut self, pipeline: VulkanPipeline) {
		self.queue_destruction(&mut [VulkanDestructor::PipelineLayout(pipeline.pipeline_layout), VulkanDestructor::Pipeline(pipeline.pipeline)]);
	}
}

impl VulkanSwapchain {}

impl VulkanGraphicsContext {
	pub fn bind_raster_pipeline(&self, pipeline: &VulkanPipeline) {
		self.queue_raster_cmd(VulkanRasterCmd::BindPipeline {
			bind_point: vk::PipelineBindPoint::GRAPHICS,
			pipeline: pipeline.pipeline,
		});
	}

	pub fn bind_compute_pipeline(&self, pipeline: &VulkanPipeline) {
		self.queue_raster_cmd(VulkanRasterCmd::BindPipeline {
			bind_point: vk::PipelineBindPoint::COMPUTE,
			pipeline: pipeline.pipeline,
		});
	}
}
