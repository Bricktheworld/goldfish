use super::{
	buffer::VulkanBuffer,
	device::{VulkanDestructor, VulkanDevice},
	VulkanSwapchain,
};
use crate::renderer::{DescriptorBindingType, DescriptorSetInfo};
use ash::vk;
use std::collections::HashMap;

pub type VulkanDescriptorLayout = vk::DescriptorSetLayout;

pub struct VulkanDescriptorLayoutCache {
	pub graphics_layouts: HashMap<DescriptorSetInfo, vk::DescriptorSetLayout>,
	pub compute_layouts: HashMap<DescriptorSetInfo, vk::DescriptorSetLayout>,
}

impl VulkanDevice {
	pub fn create_descriptor_layout_cache(&self) -> VulkanDescriptorLayoutCache {
		VulkanDescriptorLayoutCache {
			graphics_layouts: Default::default(),
			compute_layouts: Default::default(),
		}
	}

	pub fn get_graphics_layout(
		&self,
		cache: &mut VulkanDescriptorLayoutCache,
		info: DescriptorSetInfo,
	) -> VulkanDescriptorLayout {
		if let Some(layout) = cache.graphics_layouts.get(&info) {
			return *layout;
		}
		let layout = self.create_descriptor_layout(&info, vk::ShaderStageFlags::ALL_GRAPHICS);
		cache.graphics_layouts.insert(info, layout);
		layout
	}

	pub fn get_compute_layout(
		&self,
		cache: &mut VulkanDescriptorLayoutCache,
		info: DescriptorSetInfo,
	) -> VulkanDescriptorLayout {
		if let Some(layout) = cache.compute_layouts.get(&info) {
			return *layout;
		}
		let layout = self.create_descriptor_layout(&info, vk::ShaderStageFlags::COMPUTE);
		cache.compute_layouts.insert(info, layout);
		layout
	}

	pub fn destroy_descriptor_layout_cache(&mut self, cache: VulkanDescriptorLayoutCache) {
		self.queue_destruction(
			&mut cache
				.graphics_layouts
				.iter()
				.map(|(_, layout)| VulkanDestructor::DescriptorSetLayout(*layout))
				.chain(
					cache
						.compute_layouts
						.iter()
						.map(|(_, layout)| VulkanDestructor::DescriptorSetLayout(*layout)),
				)
				.collect::<Vec<_>>(),
		);
	}

	fn create_descriptor_layout(
		&self,
		info: &DescriptorSetInfo,
		stage_flags: vk::ShaderStageFlags,
	) -> vk::DescriptorSetLayout {
		unsafe {
			self.raw
				.create_descriptor_set_layout(
					&vk::DescriptorSetLayoutCreateInfo::builder().bindings(
						&info
							.bindings
							.iter()
							.map(|(binding, ty)| {
								vk::DescriptorSetLayoutBinding::builder()
									.binding(*binding)
									.descriptor_type(match *ty {
										DescriptorBindingType::Texture2D => {
											vk::DescriptorType::SAMPLED_IMAGE
										}
										DescriptorBindingType::RWTexture2D => {
											vk::DescriptorType::STORAGE_IMAGE
										}
										DescriptorBindingType::Buffer => {
											vk::DescriptorType::UNIFORM_TEXEL_BUFFER
										}
										DescriptorBindingType::RWBuffer => {
											vk::DescriptorType::STORAGE_TEXEL_BUFFER
										}
										DescriptorBindingType::SamplerState => {
											vk::DescriptorType::SAMPLER
										}
										DescriptorBindingType::CBuffer => {
											vk::DescriptorType::UNIFORM_BUFFER
										}
										DescriptorBindingType::StructuredBuffer => {
											vk::DescriptorType::STORAGE_BUFFER
										}
										DescriptorBindingType::RWStructuredBuffer => {
											vk::DescriptorType::STORAGE_BUFFER
										}
									})
									.descriptor_count(1)
									.stage_flags(stage_flags)
									.build()
							})
							.collect::<Vec<_>>(),
					),
					None,
				)
				.unwrap()
		}
	}
}

pub struct VulkanDescriptorHeap {
	pub frame_pools: [vk::DescriptorPool; VulkanSwapchain::MAX_FRAMES_IN_FLIGHT],

	pub descriptors: Vec<[vk::DescriptorSet; VulkanSwapchain::MAX_FRAMES_IN_FLIGHT]>,

	pub free_descriptors: Vec<u32>,
	pub allocated_descriptors: Vec<u32>,
}

pub struct VulkanDescriptorHandle {
	pub id: u32,
}

impl VulkanDescriptorHeap {
	pub fn alloc(&mut self) -> Option<VulkanDescriptorHandle> {
		let descriptor = self.free_descriptors.pop();
		let Some(descriptor) = descriptor else {
            return None;
        };

		self.allocated_descriptors.push(descriptor);

		Some(VulkanDescriptorHandle { id: descriptor })
	}

	pub fn free(&mut self, handle: VulkanDescriptorHandle) {
		let i = self
			.allocated_descriptors
			.iter()
			.position(|i| *i == handle.id)
			.expect("Double vulkan descriptor free detected!");
		self.allocated_descriptors.swap_remove(i);
		self.free_descriptors.push(handle.id);
	}
}

impl VulkanDevice {
	pub fn create_descriptor_heap(&self, layout: VulkanDescriptorLayout) -> VulkanDescriptorHeap {
		let max_sets = 128;
		let pool_sizes = [
			vk::DescriptorPoolSize {
				ty: vk::DescriptorType::UNIFORM_BUFFER,
				descriptor_count: max_sets * 2,
			},
			vk::DescriptorPoolSize {
				ty: vk::DescriptorType::SAMPLER,
				descriptor_count: max_sets * 4,
			},
			vk::DescriptorPoolSize {
				ty: vk::DescriptorType::SAMPLED_IMAGE,
				descriptor_count: max_sets * 4,
			},
		];

		let frame_pools = core::array::from_fn(|_| unsafe {
			self.raw
				.create_descriptor_pool(
					&vk::DescriptorPoolCreateInfo::builder()
						.pool_sizes(&pool_sizes)
						.max_sets(max_sets),
					None,
				)
				.expect("Failed to create descriptor pool!")
		});

		let descriptors = (0..max_sets)
			.map(|_| {
				core::array::from_fn(|i| unsafe {
					self.raw
						.allocate_descriptor_sets(
							&vk::DescriptorSetAllocateInfo::builder()
								.set_layouts(&[layout])
								.descriptor_pool(frame_pools[i]),
						)
						.expect("Failed to allocate descriptor set")[0]
				})
			})
			.collect::<Vec<_>>();

		let free_descriptors = (0..max_sets).map(|i| i).collect();

		VulkanDescriptorHeap {
			frame_pools,
			descriptors,
			free_descriptors,
			allocated_descriptors: Default::default(),
		}
	}

	pub fn destroy_descriptor_heap(&mut self, descriptor_heap: VulkanDescriptorHeap) {
		self.queue_destruction(
			&mut descriptor_heap
				.frame_pools
				.map(|pool| VulkanDestructor::DescriptorPool(pool)),
		);
	}
}
