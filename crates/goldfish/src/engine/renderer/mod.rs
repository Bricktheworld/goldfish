use crate::window::Window;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use uuid::uuid;

use super::package::{AssetType, Package};
use super::GoldfishEngine;
use crate::types::Color;
use backends::vulkan::*;
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use tracy_client as tracy;
pub mod backends;
pub mod render_graph;

pub const VS_MAIN: &'static str = "vs_main";
pub const PS_MAIN: &'static str = "ps_main";
pub const CS_MAIN: &'static str = "cs_main";

pub type GraphicsDevice = VulkanDevice;
pub type GraphicsContext = VulkanGraphicsContext;
pub type UploadContext = VulkanUploadContext;
pub type GpuBuffer = VulkanBuffer;
pub type Pipeline = VulkanPipeline;
pub type PipelineHandle = VulkanPipelineHandle;
pub type OutputPipelineHandle = VulkanOutputPipelineHandle;
pub type RenderPass = VulkanRenderPass;
pub type Shader = VulkanShader;
pub type Texture = VulkanTexture;
pub type Framebuffer = VulkanFramebuffer;
pub type DescriptorHeap = VulkanDescriptorHeap;
pub type DescriptorLayoutCache = VulkanDescriptorLayoutCache;
pub type DescriptorHandle = VulkanDescriptorHandle;
pub type DescriptorLayout = VulkanDescriptorLayout;

pub struct FrameId(u32);

impl FrameId {
	const FRAME_ID_MAX: u32 = 10000;
	pub fn incr(&mut self) {
		self.0 = (self.0 + 1) % Self::FRAME_ID_MAX;
	}
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum TextureFormat {
	RGB8,
	RGB16,
	RGBA8,
	RGBA16,
	SRGB8,
	SRGBA8,
	CubemapRGB8,
	CubemapRGB16,
	CubemapRGBA8,
	CubemapRGBA16,
	CubemapSRGB8,
	CubemapSRGBA8,
	Depth,
}

impl TextureFormat {
	pub fn is_cubemap(&self) -> bool {
		return (*self == TextureFormat::CubemapRGB8)
			| (*self == TextureFormat::CubemapRGB16)
			| (*self == TextureFormat::CubemapRGBA8)
			| (*self == TextureFormat::CubemapRGBA16)
			| (*self == TextureFormat::CubemapSRGB8)
			| (*self == TextureFormat::CubemapSRGBA8);
	}
}

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct TextureUsage: u16
	{
		const ATTACHMENT = 0x1;
		const TEXTURE    = 0x2;
		const STORAGE    = 0x4;
	}
}

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct BufferUsage: u16
	{
		const TransferSrc        = 0x1;
		const TransferDst        = 0x2;
		const UniformTexelBuffer = 0x4;
		const StorageTexelBuffer = 0x8;
		const UniformBuffer      = 0x10;
		const StorageBuffer      = 0x20;
		const IndexBuffer        = 0x40;
		const VertexBuffer       = 0x80;
	}
}

pub use gpu_allocator::MemoryLocation;

#[derive(Clone, Copy)]
pub enum LoadOp {
	Load,
	Clear,
	DontCare,
}

#[derive(Clone, Copy)]
pub enum StoreOp {
	Store,
	DontCare,
}

#[derive(Clone, Copy)]
pub struct AttachmentDescription {
	pub format: TextureFormat,
	pub usage: TextureUsage,
	pub load_op: LoadOp,
	pub store_op: StoreOp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DescriptorBindingType {
	UniformBuffer,
	Sampler,
	SampledImage,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DescriptorSetInfo {
	pub bindings: im::hashmap::HashMap<u32, DescriptorBindingType>,
}

impl DescriptorSetInfo {
	pub fn merge(self, other: DescriptorSetInfo) -> Self {
		let bindings = im::hashmap::HashMap::from_iter(
			self.bindings.into_iter().chain(other.bindings.into_iter()),
		);
		Self { bindings }
	}
}

use crate::types::{Vec2Serde, Vec3Serde};
#[repr(C)]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Vertex {
	#[serde(with = "Vec3Serde")]
	pub position: Vec3,
	#[serde(with = "Vec3Serde")]
	pub normal: Vec3,
	#[serde(with = "Vec2Serde")]
	pub uv: Vec2,
	#[serde(with = "Vec3Serde")]
	pub tangent: Vec3,
	#[serde(with = "Vec3Serde")]
	pub bitangent: Vec3,
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

pub struct Mesh {
	pub vertex_buffer: GpuBuffer,
	pub index_buffer: GpuBuffer,
	pub index_count: u32,
}

impl UploadContext {
	pub fn create_mesh(&mut self, vertices: &[Vertex], indices: &[u16]) -> Mesh {
		tracy::span!();
		let vertex_buffer = self.create_buffer(
			std::mem::size_of::<Vertex>() * vertices.len(),
			MemoryLocation::GpuOnly,
			BufferUsage::VertexBuffer,
			None,
			Some(bytemuck::cast_slice(vertices)),
		);

		let index_count = indices.len() as u32;
		let index_buffer = self.create_buffer(
			std::mem::size_of::<u16>() * indices.len(),
			MemoryLocation::GpuOnly,
			BufferUsage::IndexBuffer,
			None,
			Some(bytemuck::cast_slice(indices)),
		);

		Mesh {
			vertex_buffer,
			index_buffer,
			index_count,
		}
	}
}

impl GraphicsDevice {
	pub fn destroy_mesh(&mut self, mesh: Mesh) {
		tracy::span!();
		self.destroy_buffer(mesh.vertex_buffer);
		self.destroy_buffer(mesh.index_buffer);
	}
}

impl GraphicsContext {
	pub fn draw_mesh(&self, mesh: &Mesh) {
		self.bind_vertex_buffer(&mesh.vertex_buffer);
		self.bind_index_buffer(&mesh.index_buffer);
		self.draw_indexed(mesh.index_count);
	}
}

// pub enum DescriptorSetBinding<'a> {
// 	Image(&'a Texture),
// 	Buffer(&'a GpuBuffer),
// }

pub struct Renderer {
	pub graphics_device: GraphicsDevice,
	pub graphics_context: GraphicsContext,
	pub upload_context: UploadContext,
	// vertex_shader: Shader,
	// pixel_shader: Shader,
	// render_pass: RenderPass,
	// pipeline_handle: OutputPipelineHandle,
	// cube: Mesh,
	// descriptor_heap: DescriptorHeap,
	// camera_descriptor: DescriptorSet,
}

// impl Renderer {
// 	pub fn new(window: &Window, engine: &GoldfishEngine) -> Self {
// 		// let Package::Shader(shader_package) = engine.read_package(
// 		// 	uuid!("bedc27e1-f561-4c8d-bb96-6b11926b4ec8"),
// 		// 	AssetType::Shader,
// 		// ).expect("Failed to load shader package!") else
// 		// {
// 		//           panic!("Incorrect package type loaded?");
// 		// };

// 		// let vertex_shader =
// 		// 	graphics_device.create_shader(&shader_package.vs_ir.expect("No vertex shader!"));

// 		// let pixel_shader =
// 		// 	graphics_device.create_shader(&shader_package.ps_ir.expect("No vertex shader!"));

// 		// let render_pass = graphics_device.create_render_pass(
// 		// 	&[AttachmentDescription {
// 		// 		format: TextureFormat::RGBA8,
// 		// 		usage: TextureUsage::ATTACHMENT,
// 		// 		load_op: LoadOp::Clear,
// 		// 		store_op: StoreOp::Store,
// 		// 	}],
// 		// 	None,
// 		// );

// 		// let pipeline_handle =
// 		// 	graphics_context.create_raster_pipeline(&vertex_shader, &pixel_shader, true, true, 0);

// 		// let descriptor_layout = graphics_context
// 		// 	.get_raster_pipeline(pipeline_handle)
// 		// 	.unwrap()
// 		// 	.get_descriptor_layout(0);

// 		// let mut descriptor_heap = graphics_device.create_descriptor_heap(descriptor_layout);
// 		// let camera_descriptor = descriptor_heap.alloc().unwrap();

// 		Self {
// 			graphics_device,
// 			graphics_context,
// 			upload_context,
// 			// vertex_shader,
// 			// pixel_shader,
// 			// render_pass,
// 			// pipeline_handle,
// 			// cube,
// 			// descriptor_heap,
// 			// camera_descriptor,
// 		}
// 	}
// }
