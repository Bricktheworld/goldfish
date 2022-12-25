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

use crate::renderer::FrameId;
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
pub use descriptor::{
	VulkanDescriptorHandle, VulkanDescriptorHeap, VulkanDescriptorLayout,
	VulkanDescriptorLayoutCache,
};
pub use device::{VulkanDevice, VulkanUploadContext};
pub use framebuffer::VulkanFramebuffer;
pub use pipeline::{VulkanOutputPipelineHandle, VulkanPipeline, VulkanPipelineHandle};
pub use render_pass::VulkanRenderPass;
pub use shader::VulkanShader;
pub use texture::VulkanTexture;

pub enum VulkanRasterCmd {
	BindPipeline(vk::PipelineBindPoint, vk::Pipeline),
	BindVertexBuffer(u32, vk::Buffer, vk::DeviceSize),
	BindVertexBuffers(u32, Vec<vk::Buffer>, Vec<vk::DeviceSize>),
	BindIndexBuffer(vk::Buffer, vk::DeviceSize, vk::IndexType),
	SetViewport(vk::Viewport),
	SetScissor(vk::Rect2D),
	BeginRenderPass(
		vk::RenderPass,
		vk::Framebuffer,
		vk::Rect2D,
		vk::ClearValue,
		vk::SubpassContents,
	),
	EndRenderPass(),
	DrawIndexed(u32, u32, u32, i32, u32),
	BindDescriptor(
		vk::PipelineBindPoint,
		vk::PipelineLayout,
		u32,
		vk::DescriptorSet,
	),
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

impl VulkanGraphicsContext {
	pub fn begin_frame(&mut self, window: &Window) -> Result<(), SwapchainError> {
		assert!(
			self.current_frame_info.is_none(),
			"Did not call end_frame before starting another frame!"
		);

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
			if let Err(_) = self.swapchain.submit(
				current_frame_info.image_index,
				current_frame_info.command_buffer,
			) {
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
				VulkanRasterCmd::BindPipeline(bind_point, pipeline) => {
					raw.cmd_bind_pipeline(cmd_buf, bind_point, pipeline)
				}
				VulkanRasterCmd::BindVertexBuffer(first_binding, buffer, offset) => {
					raw.cmd_bind_vertex_buffers(cmd_buf, first_binding, &[buffer], &[offset]);
				}
				VulkanRasterCmd::BindVertexBuffers(first_binding, buffers, offsets) => {
					raw.cmd_bind_vertex_buffers(cmd_buf, first_binding, &buffers, &offsets);
				}
				VulkanRasterCmd::BindIndexBuffer(buffer, offset, index_type) => {
					raw.cmd_bind_index_buffer(cmd_buf, buffer, offset, index_type);
				}
				VulkanRasterCmd::SetViewport(viewport) => {
					raw.cmd_set_viewport(cmd_buf, 0, &[viewport]);
				}
				VulkanRasterCmd::SetScissor(scissor) => {
					raw.cmd_set_scissor(cmd_buf, 0, &[scissor]);
				}
				VulkanRasterCmd::BeginRenderPass(
					render_pass,
					framebuffer,
					render_area,
					clear_value,
					subpass_contents,
				) => {
					raw.cmd_begin_render_pass(
						cmd_buf,
						&vk::RenderPassBeginInfo::builder()
							.render_pass(render_pass)
							.framebuffer(framebuffer)
							.render_area(render_area)
							.clear_values(&[clear_value]),
						subpass_contents,
					);
				}
				VulkanRasterCmd::EndRenderPass() => {
					raw.cmd_end_render_pass(cmd_buf);
				}
				VulkanRasterCmd::DrawIndexed(
					index_count,
					instance_count,
					first_index,
					vertex_offset,
					first_instance,
				) => raw.cmd_draw_indexed(
					cmd_buf,
					index_count,
					instance_count,
					first_index,
					vertex_offset,
					first_instance,
				),
				VulkanRasterCmd::BindDescriptor(
					pipeline_bind_point,
					pipeline_layout,
					first_set,
					descriptor_set,
				) => raw.cmd_bind_descriptor_sets(
					cmd_buf,
					pipeline_bind_point,
					pipeline_layout,
					first_set,
					&[descriptor_set],
					&[],
				),
				VulkanRasterCmd::None => panic!("None raster command queued!"),
			}
		});
	}

	pub fn bind_output_framebuffer(&self, color: Color) {
		tracy::span!();

		self.queue_raster_cmd(VulkanRasterCmd::SetViewport(
			vk::Viewport::builder()
				.x(0.0)
				.y(self.swapchain.extent.height as f32)
				.width(self.swapchain.extent.width as f32)
				.height(-(self.swapchain.extent.height as f32))
				.min_depth(0.0)
				.max_depth(1.0)
				.build(),
		));

		self.queue_raster_cmd(VulkanRasterCmd::SetScissor(
			vk::Rect2D::builder()
				.offset(vk::Offset2D { x: 0, y: 0 })
				.extent(self.swapchain.extent)
				.build(),
		));

		self.queue_raster_cmd(VulkanRasterCmd::BeginRenderPass(
			self.swapchain.render_pass,
			self.get_output_framebuffer(),
			vk::Rect2D {
				offset: vk::Offset2D { x: 0, y: 0 },
				extent: self.swapchain.extent,
			},
			vk::ClearValue {
				color: vk::ClearColorValue {
					float32: [color.r, color.g, color.b, color.a],
				},
			},
			vk::SubpassContents::INLINE,
		));
	}

	pub fn unbind_output_framebuffer(&self) {
		self.queue_raster_cmd(VulkanRasterCmd::EndRenderPass());
	}

	pub fn end_render_pass(&self) {
		self.queue_raster_cmd(VulkanRasterCmd::EndRenderPass());
	}

	fn get_output_framebuffer(&self) -> vk::Framebuffer {
		self.current_frame_info
			.as_ref()
			.expect("begin_frame was not called!")
			.output_framebuffer
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
		ps: &VulkanShader,
		descriptor_layouts: &[VulkanDescriptorLayout],
		depth_write: bool,
		face_cull: bool,
		push_constant_bytes: usize,
	) -> VulkanOutputPipelineHandle {
		self.swapchain.create_raster_pipeline(
			vs,
			ps,
			descriptor_layouts,
			depth_write,
			face_cull,
			push_constant_bytes,
		)
	}

	pub fn get_raster_pipeline(
		&self,
		pipeline_handle: VulkanOutputPipelineHandle,
	) -> Option<&VulkanPipeline> {
		self.swapchain.get_raster_pipeline(pipeline_handle)
	}

	pub fn destroy_raster_pipeline(&mut self, pipeline_handle: VulkanOutputPipelineHandle) {
		self.swapchain.destroy_raster_pipeline(pipeline_handle);
	}

	pub fn draw_indexed(&self, index_count: u32) {
		self.queue_raster_cmd(VulkanRasterCmd::DrawIndexed(index_count, 1, 0, 0, 0));
	}

	pub fn bind_graphics_descriptor(
		&self,
		descriptor_heap: &VulkanDescriptorHeap,
		descriptor_set: &VulkanDescriptorHandle,
		set: u32,
		pipeline: &VulkanPipeline,
	) {
		let frame = self
			.current_frame_info
			.as_ref()
			.expect("begin_frame was not called!")
			.frame_index;

		let descriptor = descriptor_heap.descriptors[descriptor_set.id as usize][frame];
		self.queue_raster_cmd(VulkanRasterCmd::BindDescriptor(
			vk::PipelineBindPoint::GRAPHICS,
			pipeline.pipeline_layout,
			set,
			descriptor,
		));
	}

	pub fn write_uniform_buffers(
		&mut self,
		buffers: &[(u32, &VulkanBuffer)],
		descriptor_heap: &VulkanDescriptorHeap,
		descriptor_set: &VulkanDescriptorHandle,
	) {
		let frame = self
			.current_frame_info
			.as_ref()
			.expect("begin_frame was not called!")
			.frame_index;
		let descriptor = descriptor_heap.descriptors[descriptor_set.id as usize][frame];

		let buffer_infos = buffers
			.iter()
			.map(|(_, buffer)| {
				vk::DescriptorBufferInfo::builder()
					.buffer(buffer.raw)
					.offset(0)
					.range(buffer.size as u64)
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
							.descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
							.buffer_info(&buffer_infos[i..=i])
							.build()
					})
					.collect::<Vec<_>>(),
				&[],
			)
		};
	}
}

use crate::renderer::TextureFormat;

impl TextureFormat {
	fn to_vk(&self, device: &VulkanDevice) -> vk::Format {
		match self {
			TextureFormat::RGB8 | TextureFormat::CubemapRGB8 => vk::Format::R8G8B8_UNORM,
			TextureFormat::RGB16 | TextureFormat::CubemapRGB16 => vk::Format::R16G16B16_UNORM,
			TextureFormat::RGBA8 | TextureFormat::CubemapRGBA8 => vk::Format::R8G8B8A8_UNORM,

			TextureFormat::RGBA16 | TextureFormat::CubemapRGBA16 => vk::Format::R16G16B16A16_UNORM,
			TextureFormat::SRGB8 | TextureFormat::CubemapSRGB8 => vk::Format::R8G8B8_SRGB,
			TextureFormat::SRGBA8 | TextureFormat::CubemapSRGBA8 => vk::Format::R8G8B8A8_SRGB,
			TextureFormat::Depth => device.depth_format,
		}
	}
}
