use super::device::{VulkanDevice, VulkanDeviceChild};
use ash::vk;

pub enum QueueType
{
	GRAPHICS,
	COMPUTE,
}

pub type VulkanCommandBuffer = vk::CommandBuffer;

pub struct VulkanCommandPool
{
	command_pool: vk::CommandPool,
	command_buffers: Vec<VulkanCommandBuffer>,
	index: usize,
	destroyed: bool,
}

impl VulkanCommandPool
{
	pub fn new(device: &VulkanDevice, queue_type: QueueType) -> Self
	{
		let queue_index = match queue_type
		{
			QueueType::GRAPHICS => device.get_queue_family_indices().graphics_family,
			QueueType::COMPUTE => device.get_queue_family_indices().compute_family,
		};
		let command_pool = unsafe {
			device
				.raw
				.create_command_pool(
					&vk::CommandPoolCreateInfo::builder().queue_family_index(queue_index),
					None,
				)
				.unwrap()
		};

		Self {
			command_pool,
			command_buffers: vec![],
			index: 0,
			destroyed: false,
		}
	}

	pub fn begin_command_buffer(&mut self, device: &VulkanDevice) -> VulkanCommandBuffer
	{
		assert!(
			self.index <= self.command_buffers.len(),
			"Invalid command buffer index!"
		);

		if self.index == self.command_buffers.len()
		{
			self.expand(device);
		}

		let command_buffer = self.command_buffers[self.index];

		unsafe {
			device
				.raw
				.begin_command_buffer(
					command_buffer,
					&vk::CommandBufferBeginInfo::builder()
						.flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
				)
				.expect("Failed to begin command buffer!");
		}

		return command_buffer;
	}

	pub fn end_command_buffer(&mut self, device: &VulkanDevice, command_buffer: VulkanCommandBuffer)
	{
		unsafe {
			device
				.raw
				.end_command_buffer(command_buffer)
				.expect("Failed to end command buffer!");
		}

		self.index += 1;
	}

	pub fn recycle(&mut self, device: &VulkanDevice)
	{
		unsafe {
			device
				.raw
				.reset_command_pool(
					self.command_pool,
					vk::CommandPoolResetFlags::RELEASE_RESOURCES,
				)
				.expect("Failed to recycle command pool!");
		}
		self.index = 0;
	}

	fn expand(&mut self, device: &VulkanDevice)
	{
		let new_cmd_buffer = *unsafe {
			device
				.raw
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

		self.command_buffers.push(new_cmd_buffer);
	}
}

impl VulkanDeviceChild for VulkanCommandPool
{
	fn destroy(mut self, device: &VulkanDevice)
	{
		unsafe { device.raw.destroy_command_pool(self.command_pool, None) }
		self.destroyed = true;
	}
}

impl Drop for VulkanCommandPool
{
	fn drop(&mut self)
	{
		assert!(
			self.destroyed,
			"destroy(&VulkanDevice) was not called before VulkanCommandPool was dropped!"
		);
	}
}
