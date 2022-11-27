use super::device::VulkanDevice;
use crate::renderer::{TextureFormat, TextureUsage};
use ash::vk;
use gpu_allocator::vulkan as vma;
use gpu_allocator::MemoryLocation;

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
}

impl VulkanTexture {}

impl VulkanDevice
{
	pub fn create_texture(
		&self,
		width: u32,
		height: u32,
		format: TextureFormat,
		usage: TextureUsage,
	) -> VulkanTexture
	{
		let mut usage_flags = vk::ImageUsageFlags::default();

		if usage.contains(TextureUsage::ATTACHMENT)
		{
			if format == TextureFormat::Depth
			{
				usage_flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
			}
			else
			{
				usage_flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
			}
		}

		if usage.contains(TextureUsage::TEXTURE)
		{
			usage_flags |= vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST;
		}

		if usage.contains(TextureUsage::STORAGE)
		{
			usage_flags |= vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_DST;
		}

		let mut guard = self.vma.lock().unwrap();
		let vma = guard.as_mut().unwrap();

		let vk_format = format.to_vk(self);

		let image = unsafe {
			self.raw
				.create_image(
					&vk::ImageCreateInfo::builder()
						.flags(
							if format.is_cubemap()
							{
								vk::ImageCreateFlags::CUBE_COMPATIBLE
							}
							else
							{
								vk::ImageCreateFlags::default()
							},
						)
						.image_type(vk::ImageType::TYPE_2D)
						.format(vk_format)
						.extent(vk::Extent3D {
							width,
							height,
							depth: 1,
						})
						.mip_levels(1)
						.array_layers(if format.is_cubemap() { 6 } else { 1 })
						.samples(vk::SampleCountFlags::TYPE_1)
						.tiling(vk::ImageTiling::OPTIMAL)
						.usage(usage_flags)
						.sharing_mode(vk::SharingMode::EXCLUSIVE)
						.initial_layout(vk::ImageLayout::UNDEFINED),
					None,
				)
				.expect("Failed to create image!")
		};

		let requirements = unsafe { self.raw.get_image_memory_requirements(image) };

		let allocation = vma
			.allocate(&vma::AllocationCreateDesc {
				name: "Texture",
				requirements,
				location: MemoryLocation::GpuOnly,
				linear: false,
			})
			.expect("Failed to allocate memory!");

		unsafe {
			self.raw
				.bind_image_memory(image, allocation.memory(), allocation.offset())
				.expect("Failed to bind image memory!");
		}

		let sampler = unsafe {
			self.raw
				.create_sampler(
					&vk::SamplerCreateInfo::builder()
						.mag_filter(vk::Filter::LINEAR)
						.min_filter(vk::Filter::LINEAR)
						.mipmap_mode(vk::SamplerMipmapMode::LINEAR)
						.address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
						.address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
						.address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
						.mip_lod_bias(0.0)
						.max_anisotropy(1.0)
						.min_lod(0.0)
						.max_lod(0.0)
						.border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE),
					None,
				)
				.expect("Failed to create sampler!")
		};

		let image_view = unsafe {
			self.raw
				.create_image_view(
					&vk::ImageViewCreateInfo::builder()
						.image(image)
						.view_type(
							if format.is_cubemap()
							{
								vk::ImageViewType::CUBE
							}
							else
							{
								vk::ImageViewType::TYPE_2D
							},
						)
						.format(vk_format)
						.subresource_range(
							vk::ImageSubresourceRange::builder()
								.aspect_mask(match format
								{
									TextureFormat::Depth => vk::ImageAspectFlags::DEPTH,
									_ => vk::ImageAspectFlags::COLOR,
								})
								.base_mip_level(0)
								.level_count(1)
								.base_array_layer(0)
								.layer_count(if format.is_cubemap() { 6 } else { 1 })
								.build(),
						),
					None,
				)
				.expect("Failed to create image view!")
		};

		VulkanTexture {
			width,
			height,

			image,
			sampler,
			image_view,
			layout: vk::ImageLayout::GENERAL,

			allocation,
			format,
			usage,
		}
	}

	pub fn destroy_texture(&self, texture: VulkanTexture)
	{
		unsafe {
			self.raw.destroy_image(texture.image, None);
			self.raw.destroy_image_view(texture.image_view, None);
			self.raw.destroy_sampler(texture.sampler, None);
		}

		let mut guard = self.vma.lock().unwrap();
		let vma = guard.as_mut().unwrap();

		vma.free(texture.allocation)
			.expect("Failed to free allocation!");
	}
}
