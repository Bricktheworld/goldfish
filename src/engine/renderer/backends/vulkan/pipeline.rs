use super::{device::VulkanDevice, render_pass::VulkanRenderPass, shader::VulkanShader};
use crate::renderer::{Vertex, PS_MAIN, VS_MAIN};
use ash::vk;
use std::collections::{hash_map::Entry, HashMap};
use std::ffi::CString;

pub struct VulkanPipeline<'a>
{
	pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
	pub pipeline: vk::Pipeline,
	pub pipeline_layout: vk::PipelineLayout,
	pub render_pass: &'a VulkanRenderPass,
}

type DescriptorSetLayout = HashMap<u32, rspirv_reflect::DescriptorInfo>;
type StageDescriptorSetLayouts = HashMap<u32, DescriptorSetLayout>;

impl VulkanDevice
{
	pub fn create_raster_pipeline<'a>(
		&self,
		vs: &'a VulkanShader,
		ps: &'a VulkanShader,
		render_pass: &'a VulkanRenderPass,
		depth_write: bool,
		face_cull: bool,
		push_constant_bytes: usize,
	) -> VulkanPipeline<'a>
	{
		let shaders = [vs, ps];
		let stage_layouts = shaders
			.iter()
			.map(|shader| {
				let bytes: Vec<u8> = shader
					.code
					.iter()
					.flat_map(|code| code.to_ne_bytes())
					.collect();

				rspirv_reflect::Reflection::new_from_spirv(&bytes)
					.expect("Failed to reflect shader")
					.get_descriptor_sets()
					.unwrap()
			})
			.collect::<Vec<_>>();

		// TODO(Brandon): See if we need to use the descriptor types, this is mostly taken from kajiya
		let (descriptor_set_layouts, descriptor_types): (Vec<_>, Vec<_>) = self
			.create_descriptor_set_layouts(
				&merge_stage_layouts(stage_layouts),
				vk::ShaderStageFlags::ALL_GRAPHICS,
			)
			.iter()
			.cloned()
			.unzip();

		let mut layout_create_info =
			vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts);

		let push_constant_range = vk::PushConstantRange {
			stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS,
			offset: 0,
			size: push_constant_bytes as _,
		};

		if push_constant_bytes > 0
		{
			layout_create_info =
				layout_create_info.push_constant_ranges(std::slice::from_ref(&push_constant_range));
		}

		let pipeline_layout = unsafe {
			self.raw
				.create_pipeline_layout(&layout_create_info, None)
				.expect("Failed to create pipeline layout!")
		};

		let entry_names = vec![
			CString::new(VS_MAIN).unwrap(),
			CString::new(PS_MAIN).unwrap(),
		];
		let shader_stage_infos = [
			vk::PipelineShaderStageCreateInfo::builder()
				.module(vs.module)
				.stage(vk::ShaderStageFlags::VERTEX)
				.name(&entry_names[0])
				.build(),
			vk::PipelineShaderStageCreateInfo::builder()
				.module(ps.module)
				.stage(vk::ShaderStageFlags::FRAGMENT)
				.name(&entry_names[1])
				.build(),
		];
		let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
			.vertex_binding_descriptions(&[vk::VertexInputBindingDescription::builder()
				.binding(0)
				.stride(std::mem::size_of::<Vertex>() as u32)
				.build()])
			// TODO(Brandon): Don't hard-code this.
			.vertex_attribute_descriptions(&[
				vk::VertexInputAttributeDescription {
					location: 0,
					binding: 0,
					format: vk::Format::R32G32B32_SFLOAT,
					offset: memoffset::offset_of!(Vertex, position) as u32,
				},
				vk::VertexInputAttributeDescription {
					location: 1,
					binding: 0,
					format: vk::Format::R32G32B32_SFLOAT,
					offset: memoffset::offset_of!(Vertex, normal) as u32,
				},
				vk::VertexInputAttributeDescription {
					location: 2,
					binding: 0,
					format: vk::Format::R32G32_SFLOAT,
					offset: memoffset::offset_of!(Vertex, uv) as u32,
				},
				vk::VertexInputAttributeDescription {
					location: 3,
					binding: 0,
					format: vk::Format::R32G32B32_SFLOAT,
					offset: memoffset::offset_of!(Vertex, tangent) as u32,
				},
				vk::VertexInputAttributeDescription {
					location: 4,
					binding: 0,
					format: vk::Format::R32G32B32_SFLOAT,
					offset: memoffset::offset_of!(Vertex, bitangent) as u32,
				},
			])
			.build();

		let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
			.topology(vk::PrimitiveTopology::TRIANGLE_LIST)
			.build();

		let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
			.viewport_count(1)
			.scissor_count(1)
			.build();

		let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
			front_face: vk::FrontFace::COUNTER_CLOCKWISE,
			line_width: 1.0,
			polygon_mode: vk::PolygonMode::FILL,
			cull_mode: if face_cull
			{
				vk::CullModeFlags::BACK
			}
			else
			{
				vk::CullModeFlags::NONE
			},
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
			depth_test_enable: 1,
			depth_write_enable: if depth_write { 1 } else { 0 },
			depth_compare_op: vk::CompareOp::GREATER_OR_EQUAL,
			front: noop_stencil_state,
			back: noop_stencil_state,
			max_depth_bounds: 1.0,
			..Default::default()
		};

		let color_attachment_count = render_pass.color_attachments.len();

		let color_blend_attachment_states = vec![
			vk::PipelineColorBlendAttachmentState {
				blend_enable: 0,
				src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
				dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
				color_blend_op: vk::BlendOp::ADD,
				src_alpha_blend_factor: vk::BlendFactor::ZERO,
				dst_alpha_blend_factor: vk::BlendFactor::ZERO,
				alpha_blend_op: vk::BlendOp::ADD,
				color_write_mask: vk::ColorComponentFlags::R
					| vk::ColorComponentFlags::G
					| vk::ColorComponentFlags::B
					| vk::ColorComponentFlags::A,
			};
			color_attachment_count
		];
		let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
			.attachments(&color_blend_attachment_states);

		let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
		let dynamic_state_info =
			vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

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
			.render_pass(render_pass.raw);
		let pipeline = unsafe {
			self.raw
				.create_graphics_pipelines(
					vk::PipelineCache::null(),
					&[graphics_pipeline_info.build()],
					None,
				)
				.expect("Failed to create graphics pipeline!")
		}[0];

		VulkanPipeline {
			descriptor_set_layouts,
			pipeline,
			pipeline_layout,
			render_pass,
		}
	}

	pub fn destroy_raster_pipeline(&self, pipeline: VulkanPipeline)
	{
		unsafe {
			for layout in pipeline.descriptor_set_layouts
			{
				self.raw.destroy_descriptor_set_layout(layout, None);
			}

			self.raw.destroy_pipeline(pipeline.pipeline, None);
			self.raw
				.destroy_pipeline_layout(pipeline.pipeline_layout, None);
		}
	}

	fn create_descriptor_set_layouts(
		&self,
		descriptor_sets: &StageDescriptorSetLayouts,
		stage_flags: vk::ShaderStageFlags,
	) -> Vec<(vk::DescriptorSetLayout, HashMap<u32, vk::DescriptorType>)>
	{
		let set_count = descriptor_sets
			.iter()
			.map(|(set_index, _)| *set_index + 1)
			.max()
			.unwrap_or(0u32);

		(0..set_count)
			.map(|set_index| {
				if let Some(set) = descriptor_sets.get(&set_index)
				{
					let bindings: Vec<vk::DescriptorSetLayoutBinding> =
						set.iter()
							.map(|(binding_index, binding)| match binding.ty
							{
								rspirv_reflect::DescriptorType::UNIFORM_BUFFER
								| rspirv_reflect::DescriptorType::UNIFORM_TEXEL_BUFFER
								| rspirv_reflect::DescriptorType::STORAGE_IMAGE
								| rspirv_reflect::DescriptorType::STORAGE_BUFFER
								| rspirv_reflect::DescriptorType::STORAGE_BUFFER_DYNAMIC =>
								{
									vk::DescriptorSetLayoutBinding::builder()
									.binding(*binding_index)
									.descriptor_count(1) // TODO
									.descriptor_type(match binding.ty
									{
                                        rspirv_reflect::DescriptorType::UNIFORM_BUFFER => {
                                            vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC
                                        }
                                        rspirv_reflect::DescriptorType::UNIFORM_TEXEL_BUFFER => {
                                            vk::DescriptorType::UNIFORM_TEXEL_BUFFER
                                        }
                                        rspirv_reflect::DescriptorType::STORAGE_IMAGE => {
                                            vk::DescriptorType::STORAGE_IMAGE
                                        }
                                        // TODO
                                        // rspirv_reflect::DescriptorType::STORAGE_BUFFER => { 
                                        //     if binding.name.ends_with("_dyn") {
                                        //         vk::DescriptorType::STORAGE_BUFFER_DYNAMIC
                                        //     } else {
                                        //         vk::DescriptorType::STORAGE_BUFFER
                                        //     }
                                        // }
                                        rspirv_reflect::DescriptorType::STORAGE_BUFFER_DYNAMIC => {
                                            vk::DescriptorType::STORAGE_BUFFER_DYNAMIC
                                        }
                                        _ => unimplemented!("{:?}", binding),
									})
									.stage_flags(stage_flags)
									.build()
								}
								rspirv_reflect::DescriptorType::SAMPLED_IMAGE =>
								{
									let descriptor_count = match binding.dimensionality
									{
										rspirv_reflect::DescriptorDimensionality::Single => 1,
										rspirv_reflect::DescriptorDimensionality::Array(size) =>
										{
											size
										}
										rspirv_reflect::DescriptorDimensionality::RuntimeArray =>
										{
											unimplemented!("Bindless descriptors not implemented!")
										}
									};

									vk::DescriptorSetLayoutBinding::builder()
										.binding(*binding_index)
										.descriptor_count(descriptor_count)
										.descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
										.stage_flags(stage_flags)
										.build()
								}
								rspirv_reflect::DescriptorType::SAMPLER =>
								{
									// TODO
									// let name_prefix = "sampler_";
									// if let Some(spec) = binding.name.strip_prefix(name_prefix)
									// {
									// let texel_filter = match &spec[..1]
									// {
									// 	"n" => vk::Filter::NEAREST,
									// 	"l" => vk::Filter::LINEAR,
									// 	_ => panic!("{}", &spec[..1]),
									// };
									// spec = &spec[1..];

									// let mipmap_mode = match &spec[..1]
									// {
									// 	"n" => vk::SamplerMipmapMode::NEAREST,
									// 	"l" => vk::SamplerMipmapMode::LINEAR,
									// 	_ => panic!("{}", &spec[..1]),
									// };
									// spec = &spec[1..];

									// let address_modes = match spec
									// {
									// 	"r" => vk::SamplerAddressMode::REPEAT,
									// 	"mr" => vk::SamplerAddressMode::MIRRORED_REPEAT,
									// 	"c" => vk::SamplerAddressMode::CLAMP_TO_EDGE,
									// 	"cb" => vk::SamplerAddressMode::CLAMP_TO_BORDER,
									// 	_ => panic!("{}", spec),
									// };

									vk::DescriptorSetLayoutBinding::builder()
										.descriptor_count(1)
										.descriptor_type(vk::DescriptorType::SAMPLER)
										.stage_flags(stage_flags)
										.binding(*binding_index)
										.build()
									// }
									// else
									// {
									// 	panic!("{}", binding.name);
									// }
								}
								_ => unimplemented!("{:?}", binding),
							})
							.collect();

					let set_layout = unsafe {
						self.raw
							.create_descriptor_set_layout(
								&vk::DescriptorSetLayoutCreateInfo::builder()
									.bindings(&bindings)
									.build(),
								None,
							)
							.expect("Failed to create descriptor set layout!")
					};

					(
						set_layout,
						bindings
							.iter()
							.map(|binding| (binding.binding, binding.descriptor_type))
							.collect(),
					)
				}
				else
				{
					let set_layout = unsafe {
						self.raw
							.create_descriptor_set_layout(
								&vk::DescriptorSetLayoutCreateInfo::builder().build(),
								None,
							)
							.expect("Failed to create descriptor set layout!")
					};
					(set_layout, Default::default())
				}
			})
			.collect::<Vec<_>>()
	}
}

fn merge_stage_layouts(stages: Vec<StageDescriptorSetLayouts>) -> StageDescriptorSetLayouts
{
	let mut stages = stages.into_iter();
	let mut dst = stages.next().unwrap_or_default();

	for src in stages
	{
		for (set_idx, set) in src
		{
			match dst.entry(set_idx)
			{
				Entry::Occupied(mut existing) =>
				{
					let existing = existing.get_mut();
					for (binding_idx, binding) in set
					{
						match existing.entry(binding_idx)
						{
							Entry::Occupied(existing) =>
							{
								let existing = existing.get();
								assert_eq!(
									existing.ty, binding.ty,
									"binding idx: {}, name: {:?}",
									binding_idx, binding.name
								);
								assert_eq!(
									existing.name, binding.name,
									"binding idx: {}, name: {:?}",
									binding_idx, binding.name
								);
							}
							Entry::Vacant(vacant) =>
							{
								vacant.insert(binding);
							}
						}
					}
				}
				Entry::Vacant(vacant) =>
				{
					vacant.insert(set);
				}
			}
		}
	}

	dst
}
