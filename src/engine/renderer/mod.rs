use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use backends::vulkan::{
	VulkanBuffer, VulkanDevice, VulkanGraphicsContext, VulkanPipeline, VulkanRenderPass,
	VulkanShader, VulkanTexture, VulkanUploadContext,
};
use glam::{Vec2, Vec3, Vec4};
use tracy_client as tracy;
pub mod backends;

pub const VS_MAIN: &'static str = "vs_main";
pub const PS_MAIN: &'static str = "ps_main";
pub const CS_MAIN: &'static str = "cs_main";

pub type GraphicsDevice = VulkanDevice;
pub type GraphicsContext = VulkanGraphicsContext;
pub type UploadContext = VulkanUploadContext;
pub type GpuBuffer = VulkanBuffer;
pub type Pipeline<'a> = VulkanPipeline<'a>;
pub type RenderPass = VulkanRenderPass;
pub type Shader = VulkanShader;

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum TextureFormat
{
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

impl TextureFormat
{
	pub fn is_cubemap(&self) -> bool
	{
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
pub enum LoadOp
{
	Load,
	Clear,
	DontCare,
}

#[derive(Clone, Copy)]
pub enum StoreOp
{
	Store,
	DontCare,
}

#[derive(Clone, Copy)]
pub struct AttachmentDescription
{
	pub format: TextureFormat,
	pub load_op: LoadOp,
	pub store_op: StoreOp,
}

pub struct Texture(VulkanTexture);

use crate::types::{Vec2Serde, Vec3Serde};
#[repr(C)]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Vertex
{
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

pub struct Mesh
{
	pub vertex_buffer: GpuBuffer,
	pub index_buffer: GpuBuffer,
}

impl UploadContext
{
	pub fn create_mesh(&mut self, vertices: &[Vertex], indices: &[u16]) -> Mesh
	{
		tracy::span!();
		let vertex_buffer = self.create_buffer(
			std::mem::size_of::<Vertex>() * vertices.len(),
			MemoryLocation::GpuOnly,
			BufferUsage::VertexBuffer,
			None,
			Some(bytemuck::cast_slice(vertices)),
		);

		let index_buffer = self.create_buffer(
			2 * indices.len(),
			MemoryLocation::GpuOnly,
			BufferUsage::IndexBuffer,
			None,
			Some(bytemuck::cast_slice(indices)),
		);

		Mesh {
			vertex_buffer,
			index_buffer,
		}
	}
}

impl VulkanDevice
{
	pub fn destroy_mesh(&self, mesh: Mesh)
	{
		tracy::span!();
		self.destroy_buffer(mesh.vertex_buffer);
		self.destroy_buffer(mesh.index_buffer);
	}
}
