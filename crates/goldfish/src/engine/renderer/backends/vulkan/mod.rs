mod buffer;
mod command_pool;
mod descriptor;
mod device;
mod fence;
mod framebuffer;
mod pipeline;
mod render_pass;
mod semaphore;
mod shader;
mod swapchain;
mod texture;

use crate::window::Window;
use command_pool::VulkanCommandBuffer;
use swapchain::{FrameInfo, VulkanSwapchain};

use crate::renderer::{ClearValue, DepthCompareOp, DescriptorSetInfo, FaceCullMode, FrameId, ImageLayout, PolygonMode, VertexInputInfo};
use crate::types::{Color, Size};
use ash::vk;
use custom_error::custom_error;
use std::cell::RefCell;
use tracy_client as tracy;

custom_error! {pub SwapchainError
	SubmitSuboptimal = "Swapchain is suboptimal and needs to be recreated",
	AcquireSuboptimal = "Swapchain is suboptimal and needs to be recreated"
}

pub use buffer::VulkanBuffer;
pub use descriptor::{VulkanDescriptorHandle, VulkanDescriptorHeap, VulkanDescriptorLayout, VulkanDescriptorLayoutCache};
pub use device::{VulkanDevice, VulkanUploadContext};
pub use framebuffer::VulkanFramebuffer;
pub use pipeline::VulkanPipeline;
pub use render_pass::VulkanRenderPass;
pub use shader::VulkanShader;
pub use texture::VulkanTexture;

pub enum VulkanRasterCmd {
	BindPipeline {
		bind_point: vk::PipelineBindPoint,
		pipeline: vk::Pipeline,
	},
	BindVertexBuffer {
		first_binding: u32,
		buffer: vk::Buffer,
		offset: vk::DeviceSize,
	},
	BindVertexBuffers {
		first_binding: u32,
		buffers: Vec<vk::Buffer>,
		offsets: Vec<vk::DeviceSize>,
	},
	BindIndexBuffer {
		buffer: vk::Buffer,
		offset: vk::DeviceSize,
		index_type: vk::IndexType,
	},
	SetViewport {
		viewport: vk::Viewport,
	},
	SetScissor {
		scissor: vk::Rect2D,
	},
	BeginRenderPass {
		render_pass: vk::RenderPass,
		framebuffer: vk::Framebuffer,
		render_area: vk::Rect2D,
		clear_values: Vec<vk::ClearValue>,
		subpass_contents: vk::SubpassContents,
	},
	EndRenderPass {},
	DrawIndexed {
		index_count: u32,
		instance_count: u32,
		first_index: u32,
		vertex_offset: i32,
		first_instance: u32,
	},
	Draw {
		vertex_count: u32,
		instance_count: u32,
		first_vertex: u32,
		first_instance: u32,
	},
	BindDescriptor {
		pipeline_bind_point: vk::PipelineBindPoint,
		pipeline_layout: vk::PipelineLayout,
		first_set: u32,
		descriptor_set: vk::DescriptorSet,
	},
	PipelineBarrier {
		src_stage_mask: vk::PipelineStageFlags,
		dst_stage_mask: vk::PipelineStageFlags,
		dependency_flags: vk::DependencyFlags,
		memory_barriers: Vec<vk::MemoryBarrier>,
		buffer_memory_barriers: Vec<vk::BufferMemoryBarrier>,
		image_memory_barriers: Vec<vk::ImageMemoryBarrier>,
	},
	Dispatch {
		group_count_x: u32,
		group_count_y: u32,
		group_count_z: u32,
	},
	None,
}

impl Default for VulkanRasterCmd {
	fn default() -> Self {
		Self::None
	}
}

pub struct VulkanUniformBufferUpdate {
	pub buffer: vk::Buffer,
	pub offset: usize,
	pub range: usize,
}

impl VulkanDevice {
	pub fn new_with_context(window: &Window) -> (Self, VulkanGraphicsContext) {
		let device = VulkanDevice::new(window);
		let swapchain = VulkanSwapchain::new(window.get_size(), device.clone());

		(
			device,
			VulkanGraphicsContext {
				swapchain,
				current_frame_info: None,
				raster_cmds: Default::default(),
				frame_id: FrameId(0),
			},
		)
	}
}

pub struct VulkanGraphicsContext {
	swapchain: VulkanSwapchain,
	current_frame_info: Option<FrameInfo>,
	raster_cmds: RefCell<Vec<VulkanRasterCmd>>,
	frame_id: FrameId,
}

impl From<ClearValue> for vk::ClearValue {
	fn from(c: ClearValue) -> Self {
		match c {
			ClearValue::Color { r, g, b, a } => vk::ClearValue {
				color: vk::ClearColorValue { float32: [r, g, b, a] },
			},
			ClearValue::DepthStencil { depth, stencil } => vk::ClearValue {
				depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
			},
		}
	}
}

impl VulkanGraphicsContext {
	pub fn begin_frame(&mut self, window: &Window) -> Result<(), SwapchainError> {
		assert!(self.current_frame_info.is_none(), "Did not call end_frame before starting another frame!");

		self.frame_id.incr();
		match self.swapchain.acquire() {
			Ok(res) => {
				self.current_frame_info = Some(res);

				Ok(())
			}
			Err(err) => {
				self.swapchain.invalidate(window.get_size());
				Err(err)
			}
		}
	}

	pub fn end_frame(&mut self, window: &Window) {
		if let Some(current_frame_info) = self.current_frame_info.take() {
			self.fill_raster_cmds(current_frame_info.command_buffer);
			if let Err(_) = self.swapchain.submit(current_frame_info.image_index, current_frame_info.command_buffer) {
				self.swapchain.invalidate(window.get_size());
			}
		} else {
			panic!("Did not call begin_frame first!");
		}
	}

	pub fn queue_raster_cmd(&self, cmd: VulkanRasterCmd) {
		self.raster_cmds.borrow_mut().push(cmd);
	}

	fn fill_raster_cmds(&self, cmd_buf: VulkanCommandBuffer) {
		tracy::span!();
		let raw = self.raw_device();
		self.raster_cmds.take().into_iter().for_each(|cmd| unsafe {
			match cmd {
				VulkanRasterCmd::BindPipeline { bind_point, pipeline } => raw.cmd_bind_pipeline(cmd_buf, bind_point, pipeline),
				VulkanRasterCmd::BindVertexBuffer { first_binding, buffer, offset } => {
					raw.cmd_bind_vertex_buffers(cmd_buf, first_binding, &[buffer], &[offset]);
				}
				VulkanRasterCmd::BindVertexBuffers { first_binding, buffers, offsets } => {
					raw.cmd_bind_vertex_buffers(cmd_buf, first_binding, &buffers, &offsets);
				}
				VulkanRasterCmd::BindIndexBuffer { buffer, offset, index_type } => {
					raw.cmd_bind_index_buffer(cmd_buf, buffer, offset, index_type);
				}
				VulkanRasterCmd::SetViewport { viewport } => {
					raw.cmd_set_viewport(cmd_buf, 0, &[viewport]);
				}
				VulkanRasterCmd::SetScissor { scissor } => {
					raw.cmd_set_scissor(cmd_buf, 0, &[scissor]);
				}
				VulkanRasterCmd::BeginRenderPass {
					render_pass,
					framebuffer,
					render_area,
					clear_values,
					subpass_contents,
				} => {
					raw.cmd_begin_render_pass(
						cmd_buf,
						&vk::RenderPassBeginInfo::builder()
							.render_pass(render_pass)
							.framebuffer(framebuffer)
							.render_area(render_area)
							.clear_values(&clear_values),
						subpass_contents,
					);
				}
				VulkanRasterCmd::EndRenderPass {} => {
					raw.cmd_end_render_pass(cmd_buf);
				}
				VulkanRasterCmd::DrawIndexed {
					index_count,
					instance_count,
					first_index,
					vertex_offset,
					first_instance,
				} => raw.cmd_draw_indexed(cmd_buf, index_count, instance_count, first_index, vertex_offset, first_instance),
				VulkanRasterCmd::BindDescriptor {
					pipeline_bind_point,
					pipeline_layout,
					first_set,
					descriptor_set,
				} => raw.cmd_bind_descriptor_sets(cmd_buf, pipeline_bind_point, pipeline_layout, first_set, &[descriptor_set], &[]),
				VulkanRasterCmd::PipelineBarrier {
					src_stage_mask,
					dst_stage_mask,
					dependency_flags,
					memory_barriers,
					buffer_memory_barriers,
					image_memory_barriers,
				} => raw.cmd_pipeline_barrier(
					cmd_buf,
					src_stage_mask,
					dst_stage_mask,
					dependency_flags,
					&memory_barriers,
					&buffer_memory_barriers,
					&image_memory_barriers,
				),
				VulkanRasterCmd::Draw {
					vertex_count,
					instance_count,
					first_vertex,
					first_instance,
				} => raw.cmd_draw(cmd_buf, vertex_count, instance_count, first_vertex, first_instance),
				VulkanRasterCmd::Dispatch {
					group_count_x,
					group_count_y,
					group_count_z,
				} => raw.cmd_dispatch(cmd_buf, group_count_x, group_count_y, group_count_z),
				VulkanRasterCmd::None => panic!("None raster command queued!"),
			}
		});
	}

	pub fn begin_output_render_pass(&self, clear_values: &[ClearValue]) {
		tracy::span!();

		self.queue_raster_cmd(VulkanRasterCmd::SetViewport {
			viewport: vk::Viewport::builder()
				.x(0.0)
				.y(self.swapchain.extent.height as f32)
				.width(self.swapchain.extent.width as f32)
				.height(-(self.swapchain.extent.height as f32))
				.min_depth(0.0)
				.max_depth(1.0)
				.build(),
		});

		self.queue_raster_cmd(VulkanRasterCmd::SetScissor {
			scissor: vk::Rect2D::builder().offset(vk::Offset2D { x: 0, y: 0 }).extent(self.swapchain.extent).build(),
		});

		self.queue_raster_cmd(VulkanRasterCmd::BeginRenderPass {
			render_pass: self.swapchain.render_pass,
			framebuffer: self.get_output_framebuffer(),
			render_area: vk::Rect2D {
				offset: vk::Offset2D { x: 0, y: 0 },
				extent: self.swapchain.extent,
			},
			clear_values: clear_values.iter().map(|&c| c.into()).collect::<Vec<_>>(),
			subpass_contents: vk::SubpassContents::INLINE,
		});
	}

	pub fn begin_render_pass(&self, render_pass: &VulkanRenderPass, framebuffer: &VulkanFramebuffer, clear_values: &[ClearValue]) {
		self.queue_raster_cmd(VulkanRasterCmd::SetViewport {
			viewport: vk::Viewport::builder()
				.x(0.0)
				.y(framebuffer.height as f32)
				.width(framebuffer.width as f32)
				.height(-(framebuffer.height as f32))
				.min_depth(0.0)
				.max_depth(1.0)
				.build(),
		});

		let extent = vk::Extent2D {
			width: framebuffer.width,
			height: framebuffer.height,
		};

		self.queue_raster_cmd(VulkanRasterCmd::SetScissor {
			scissor: vk::Rect2D::builder().offset(vk::Offset2D { x: 0, y: 0 }).extent(extent).build(),
		});

		self.queue_raster_cmd(VulkanRasterCmd::BeginRenderPass {
			render_pass: render_pass.raw,
			framebuffer: framebuffer.raw,
			render_area: vk::Rect2D {
				offset: vk::Offset2D { x: 0, y: 0 },
				extent,
			},
			clear_values: clear_values.iter().map(|&c| c.into()).collect::<Vec<_>>(),
			subpass_contents: vk::SubpassContents::INLINE,
		});
	}

	pub fn end_render_pass(&self) {
		self.queue_raster_cmd(VulkanRasterCmd::EndRenderPass {});
	}

	fn get_output_framebuffer(&self) -> vk::Framebuffer {
		self.current_frame_info.as_ref().expect("begin_frame was not called!").output_framebuffer
	}

	pub fn raw_device(&self) -> &ash::Device {
		self.swapchain.raw_device()
	}

	pub fn on_resize(&mut self, framebuffer_size: Size) {
		tracy::span!();
		self.swapchain.invalidate(framebuffer_size);
	}

	pub fn destroy(&mut self) {
		self.swapchain.destroy();
	}
	pub fn create_raster_pipeline(
		&mut self,
		vs: &VulkanShader,
		ps: Option<&VulkanShader>,
		descriptor_layouts: &[VulkanDescriptorLayout],
		depth_compare_op: Option<DepthCompareOp>,
		depth_write: bool,
		face_cull: FaceCullMode,
		push_constant_bytes: usize,
		vertex_input_info: VertexInputInfo,
		polygon_mode: PolygonMode,
	) -> VulkanPipeline {
		self.swapchain.device.create_raster_pipeline_impl(
			vs,
			ps,
			descriptor_layouts,
			self.swapchain.render_pass,
			1usize,
			depth_compare_op,
			depth_write,
			face_cull,
			push_constant_bytes,
			vertex_input_info,
			polygon_mode,
		)
	}

	pub fn draw_indexed(&self, index_count: u32) {
		self.queue_raster_cmd(VulkanRasterCmd::DrawIndexed {
			index_count,
			instance_count: 1,
			first_index: 0,
			vertex_offset: 0,
			first_instance: 0,
		});
	}

	pub fn draw(&self, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) {
		self.queue_raster_cmd(VulkanRasterCmd::Draw {
			vertex_count,
			instance_count,
			first_vertex,
			first_instance,
		});
	}

	pub fn bind_graphics_descriptor(&self, descriptor_heap: &VulkanDescriptorHeap, descriptor_set: &VulkanDescriptorHandle, set: u32, pipeline: &VulkanPipeline) {
		let frame = self.current_frame_info.as_ref().expect("begin_frame was not called!").frame_index;

		let descriptor = descriptor_heap.descriptors[descriptor_set.id as usize][frame];
		self.queue_raster_cmd(VulkanRasterCmd::BindDescriptor {
			pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
			pipeline_layout: pipeline.pipeline_layout,
			first_set: set,
			descriptor_set: descriptor,
		});
	}

	pub fn bind_compute_descriptor(&self, descriptor_heap: &VulkanDescriptorHeap, descriptor_set: &VulkanDescriptorHandle, set: u32, pipeline: &VulkanPipeline) {
		let frame = self.current_frame_info.as_ref().expect("begin_frame was not called!").frame_index;

		let descriptor = descriptor_heap.descriptors[descriptor_set.id as usize][frame];
		self.queue_raster_cmd(VulkanRasterCmd::BindDescriptor {
			pipeline_bind_point: vk::PipelineBindPoint::COMPUTE,
			pipeline_layout: pipeline.pipeline_layout,
			first_set: set,
			descriptor_set: descriptor,
		});
	}

	pub fn update_descriptor(
		&mut self,
		buffers: &[(u32, &VulkanBuffer)],
		images: &[(u32, &VulkanTexture, ImageLayout)],
		descriptor_layout: &'static DescriptorSetInfo,
		descriptor_heap: &VulkanDescriptorHeap,
		descriptor_set: &VulkanDescriptorHandle,
	) {
		let frame = self.current_frame_info.as_ref().expect("begin_frame was not called!").frame_index;
		let descriptor = descriptor_heap.descriptors[descriptor_set.id as usize][frame];

		let buffer_infos = buffers
			.iter()
			.map(|(_, buffer)| vk::DescriptorBufferInfo::builder().buffer(buffer.raw).offset(0).range(buffer.size as u64).build())
			.collect::<Vec<_>>();

		let image_infos = images
			.iter()
			.map(|(_, image, layout)| {
				vk::DescriptorImageInfo::builder()
					.image_view(image.image_view)
					.sampler(image.sampler)
					.image_layout((*layout).into())
					.build()
			})
			.collect::<Vec<_>>();

		unsafe {
			self.raw_device().update_descriptor_sets(
				&buffers
					.iter()
					.enumerate()
					.map(|(i, (binding, _))| {
						vk::WriteDescriptorSet::builder()
							.dst_set(descriptor)
							.dst_binding(*binding)
							.descriptor_type((*descriptor_layout.bindings.get(&binding).unwrap()).into())
							.buffer_info(&buffer_infos[i..=i])
							.build()
					})
					.chain(images.iter().enumerate().map(|(i, (binding, _, _))| {
						vk::WriteDescriptorSet::builder()
							.dst_set(descriptor)
							.dst_binding(*binding)
							.descriptor_type((*descriptor_layout.bindings.get(&binding).unwrap()).into())
							.image_info(&image_infos[i..=i])
							.build()
					}))
					.collect::<Vec<_>>(),
				&[],
			)
		};
	}

	pub fn pipeline_barrier(
		&self,
		src_stage_mask: vk::PipelineStageFlags,
		dst_stage_mask: vk::PipelineStageFlags,
		dependency_flags: vk::DependencyFlags,
		memory_barriers: &[vk::MemoryBarrier],
		buffer_memory_barriers: &[vk::BufferMemoryBarrier],
		image_memory_barriers: &[vk::ImageMemoryBarrier],
	) {
		self.queue_raster_cmd(VulkanRasterCmd::PipelineBarrier {
			src_stage_mask,
			dst_stage_mask,
			dependency_flags,
			memory_barriers: memory_barriers.to_vec(),
			buffer_memory_barriers: buffer_memory_barriers.to_vec(),
			image_memory_barriers: image_memory_barriers.to_vec(),
		})
	}

	pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
		self.queue_raster_cmd(VulkanRasterCmd::Dispatch {
			group_count_x,
			group_count_y,
			group_count_z,
		})
	}
}

use crate::renderer::TextureFormat;

impl TextureFormat {
	fn to_vk(&self, device: &VulkanDevice) -> vk::Format {
		match self {
			TextureFormat::R8UNorm => vk::Format::R8_UNORM,
			TextureFormat::R16UNorm => vk::Format::R16_UNORM,
			TextureFormat::RG8UNorm => vk::Format::R8G8_UNORM,
			TextureFormat::RG16UNorm => vk::Format::R16G16_UNORM,
			TextureFormat::RGB8UNorm | TextureFormat::CubemapRGB8UNorm => vk::Format::R8G8B8_UNORM,
			TextureFormat::RGB16UNorm | TextureFormat::CubemapRGB16UNorm => vk::Format::R16G16B16_UNORM,
			TextureFormat::RGBA8UNorm | TextureFormat::CubemapRGBA8UNorm => vk::Format::R8G8B8A8_UNORM,
			TextureFormat::RGBA16UNorm | TextureFormat::CubemapRGBA16UNorm => vk::Format::R16G16B16A16_UNORM,
			TextureFormat::SRGB8 | TextureFormat::CubemapSRGB8 => vk::Format::R8G8B8_SRGB,
			TextureFormat::SRGBA8 | TextureFormat::CubemapSRGBA8 => vk::Format::R8G8B8A8_SRGB,
			TextureFormat::R8SNorm => vk::Format::R8_SNORM,
			TextureFormat::R16SNorm => vk::Format::R16_SNORM,
			TextureFormat::RG8SNorm => vk::Format::R8G8_SNORM,
			TextureFormat::RG16SNorm => vk::Format::R16G16_SNORM,
			TextureFormat::RGB8SNorm => vk::Format::R8G8B8_SNORM,
			TextureFormat::RGB16SNorm => vk::Format::R16G16B16_SNORM,
			TextureFormat::RGBA8SNorm => vk::Format::R8G8B8A8_SNORM,
			TextureFormat::RGBA16SNorm => vk::Format::R16G16B16A16_SNORM,
			TextureFormat::R8UInt => vk::Format::R8_UINT,
			TextureFormat::R16UInt => vk::Format::R16_UINT,
			TextureFormat::R32UInt => vk::Format::R32_UINT,
			TextureFormat::RG8UInt => vk::Format::R8G8_UINT,
			TextureFormat::RG16UInt => vk::Format::R16G16_UINT,
			TextureFormat::RG32UInt => vk::Format::R32G32_UINT,
			TextureFormat::RGB8UInt => vk::Format::R8G8B8_UINT,
			TextureFormat::RGB16UInt => vk::Format::R16G16B16_UINT,
			TextureFormat::RGB32UInt => vk::Format::R32G32B32_UINT,
			TextureFormat::RGBA8UInt => vk::Format::R8G8B8A8_UINT,
			TextureFormat::RGBA16UInt => vk::Format::R16G16B16A16_UINT,
			TextureFormat::RGBA32UInt => vk::Format::R32G32B32A32_UINT,
			TextureFormat::R8SInt => vk::Format::R8_SINT,
			TextureFormat::R16SInt => vk::Format::R16_SINT,
			TextureFormat::R32SInt => vk::Format::R32_SINT,
			TextureFormat::RG8SInt => vk::Format::R8G8_SINT,
			TextureFormat::RG16SInt => vk::Format::R16G16_SINT,
			TextureFormat::RG32SInt => vk::Format::R32G32_SINT,
			TextureFormat::RGB8SInt => vk::Format::R8G8B8_SINT,
			TextureFormat::RGB16SInt => vk::Format::R16G16B16_SINT,
			TextureFormat::RGB32SInt => vk::Format::R32G32B32_SINT,
			TextureFormat::RGBA8SInt => vk::Format::R8G8B8A8_SINT,
			TextureFormat::RGBA16SInt => vk::Format::R16G16B16A16_SINT,
			TextureFormat::RGBA32SInt => vk::Format::R32G32B32A32_SINT,
			TextureFormat::R32Float => vk::Format::R32_SFLOAT,
			TextureFormat::RG32Float => vk::Format::R32G32_SFLOAT,
			TextureFormat::RGB32Float => vk::Format::R32G32B32_SFLOAT,
			TextureFormat::RGBA32Float => vk::Format::R32G32B32A32_SFLOAT,
			TextureFormat::Depth => device.depth_format,
		}
	}
}
