use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use backends::vulkan::{VulkanBuffer, VulkanDevice, VulkanGraphicsContext, VulkanTexture};
pub mod backends;

pub type GraphicsDevice = VulkanDevice;
pub type GraphicsContext = VulkanGraphicsContext;
pub type GpuBuffer = VulkanBuffer;

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

pub struct Texture(VulkanTexture);

pub struct Mesh
{
	index_buffer: GpuBuffer,
	vertex_buffer: GpuBuffer,
}

// impl GraphicsDevice
// {
// 	pub fn create_mesh(&self) -> Mesh
// 	{
// 		Mesh {}
// 	}
// }
