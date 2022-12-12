use super::{
	device::{VulkanDestructor, VulkanDevice},
	pipeline::VulkanDescriptorSetLayout,
	VulkanSwapchain,
};
use ash::vk;

pub struct VulkanDescriptorHeap {
	pub frame_pools: [vk::DescriptorPool; VulkanSwapchain::MAX_FRAMES_IN_FLIGHT],

	pub descriptors: Vec<[vk::DescriptorSet; VulkanSwapchain::MAX_FRAMES_IN_FLIGHT]>,

	pub free_descriptors: Vec<u32>,
	pub allocated_descriptors: Vec<u32>,
}

pub struct VulkanDescriptorSetHandle(u32);

impl VulkanDescriptorHeap {
	pub fn alloc(&mut self) -> Option<VulkanDescriptorSetHandle> {
		let descriptor = self.free_descriptors.pop();
		let Some(descriptor) = descriptor else {
            return None;
        };

		self.allocated_descriptors.push(descriptor);

		Some(VulkanDescriptorSetHandle(descriptor))
	}
}

impl VulkanDevice {
	pub fn create_descriptor_heap(
		&self,
		layout: VulkanDescriptorSetLayout,
	) -> VulkanDescriptorHeap {
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
								.set_layouts(&[layout.raw])
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

	pub fn update_descriptor(
		&mut self,
		descriptor_heap: &VulkanDescriptorHeap,
		descriptor_set: &VulkanDescriptorSetHandle,
	) {
		let frame_sets = descriptor_heap.descriptors[descriptor_set.0 as usize];
		let guard = self.frame.lock().unwrap();
	}
}

// pub struct VulkanDescriptorPool
// {
// 	pub frame_pools: [vk::DescriptorPool; VulkanSwapchain::MAX_FRAMES_IN_FLIGHT],
// }

// pub struct VulkanDescriptorSet<'a>
// {
// 	bindings: HashMap<u32, DescriptorSetBinding<'a>>,
// }

// impl VulkanDevice
// {
// 	pub fn create_descriptor_pool(&self) -> VulkanDescriptorPool
// 	{
// 		let frame_pools = core::array::from_fn(|_| unsafe {
// 			self.raw
// 				.create_descriptor_pool(
// 					&vk::DescriptorPoolCreateInfo::builder()
// 						.pool_sizes(&[
// 							vk::DescriptorPoolSize {
// 								ty: vk::DescriptorType::UNIFORM_BUFFER,
// 								descriptor_count: 512,
// 							},
// 							vk::DescriptorPoolSize {
// 								ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
// 								descriptor_count: 1024,
// 							},
// 						])
// 						.max_sets(128),
// 					None,
// 				)
// 				.expect("Failed to create descriptor pool!")
// 		});

// 		VulkanDescriptorPool { frame_pools }
// 	}

// 	pub fn destroy_descriptor_pool(&mut self, descriptor_pool: VulkanDescriptorPool)
// 	{
// 		self.queue_destruction(
// 			&mut descriptor_pool
// 				.frame_pools
// 				.map(|pool| VulkanDestructor::DescriptorPool(pool)),
// 		);
// 	}
// }
