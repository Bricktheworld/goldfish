use super::command_buffer::VulkanCommandBuffer;
use super::device::VulkanDevice;
use ash::vk;
use std::sync::Weak;

pub enum QueueType
{
	GRAPHICS,
	COMPUTE,
}

pub struct VulkanCommandPool
{
	device: Weak<VulkanDevice>,
	command_pool: vk::CommandPool,
	command_buffers: Vec<VulkanCommandBuffer>,
	index: usize,
}

impl VulkanCommandPool
{
	pub fn new(device: Weak<VulkanDevice>, queue_type: QueueType) -> Self
	{
		let dev = device.upgrade().unwrap();
		let queue_index = match queue_type
		{
			QueueType::GRAPHICS => dev.get_queue_family_indices().graphics_family,
			QueueType::COMPUTE => dev.get_queue_family_indices().compute_family,
		};
		let command_pool = unsafe {
			dev.vk_device()
				.create_command_pool(
					&vk::CommandPoolCreateInfo::builder().queue_family_index(queue_index),
					None,
				)
				.unwrap()
		};

		Self {
			device,
			command_pool,
			command_buffers: vec![],
			index: 0,
		}
	}

	pub fn begin_command_buffer<'a>(&'a mut self) -> &'a VulkanCommandBuffer
	{
		assert!(
			self.index <= self.command_buffers.len(),
			"Invalid command buffer index!"
		);

		if self.index == self.command_buffers.len()
		{
			self.expand();
		}

		let command_buffer = &self.command_buffers[self.index];

		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.begin_command_buffer(
					command_buffer.get(),
					&vk::CommandBufferBeginInfo::builder()
						.flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
				)
				.expect("Failed to begin command buffer!");
		}

		return command_buffer;
	}

	fn expand(&mut self)
	{
		let device = self.device.upgrade().unwrap();
		let new_cmd_buffer = *unsafe {
			device
				.vk_device()
				.allocate_command_buffers(
					&vk::CommandBufferAllocateInfo::builder()
						.command_pool(self.command_pool)
						.level(vk::CommandBufferLevel::PRIMARY)
						.command_buffer_count(1),
				)
				.unwrap()
		}
		.first()
		.unwrap();

		self.command_buffers
			.push(VulkanCommandBuffer::new(new_cmd_buffer));
	}
}

impl Drop for VulkanCommandPool
{
	fn drop(&mut self)
	{
		unsafe {
			self.device
				.upgrade()
				.unwrap()
				.vk_device()
				.destroy_command_pool(self.command_pool, None)
		}
	}
}
