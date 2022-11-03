use bitflags::bitflags;
pub mod backends;

use backends::vulkan::{VulkanGraphicsContext, VulkanGraphicsDevice};
pub type GraphicsDevice = VulkanGraphicsDevice;
pub type GraphicsContext = VulkanGraphicsContext;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
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
