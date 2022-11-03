use super::VulkanDevice;
use crate::renderer::{TextureFormat, TextureUsage};
use ash::vk;
use gpu_allocator::vulkan as vma;
use std::sync::{Arc, Weak};

pub struct VulkanTexture
{
	width: u32,
	height: u32,

	image: vk::Image,
	sampler: vk::Sampler,
	image_view: vk::ImageView,
	layout: vk::ImageLayout,

	allocation: vma::Allocation,
	format: TextureFormat,
	usage: TextureUsage,

	device: Weak<VulkanDevice>,
}

impl VulkanTexture
{
	// pub fn new(
	// 	device: &Arc<VulkanDevice>,
	// 	width: u32,
	// 	height: u32,
	// 	format: TextureFormat,
	// 	usage: TextureUsage,
	// ) -> Self
	// {
	// 	let usage_flags = vk::ImageUsageFlags::default();

	// 	if usage.contains(TextureUsage::ATTACHMENT)
	// 	{
	// 		if format == TextureFormat::Depth
	// 		{
	// 			usage_flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
	// 		}
	// 		else
	// 		{
	// 			usage_flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
	// 		}
	// 	}

	// 	if usage.contains(TextureUsage::TEXTURE)
	// 	{
	// 		usage_flags |= vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST;
	// 	}

	// 	if usage.contains(TextureUsage::STORAGE)
	// 	{
	// 		usage_flags |= vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_DST;
	// 	}

	// 	let vma = device.vma().unwrap();

	// 	let device = Arc::downgrade(device);

	// 	Self { device }
	// }
}

impl Drop for VulkanTexture
{
	fn drop(&mut self)
	{
		let guard = self.device.upgrade().unwrap();
		let vk_device = guard.vk_device();
		unsafe {
			vk_device.destroy_image(self.image, None);
			vk_device.destroy_image_view(self.image_view, None);
			vk_device.destroy_sampler(self.sampler, None);
		}
	}
}
