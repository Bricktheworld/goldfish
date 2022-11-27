use super::device::VulkanDevice;
use ash::vk;
use tracy_client as tracy;

pub enum QueueType
{
	GRAPHICS,
	COMPUTE,
}

pub type VulkanCommandBuffer = vk::CommandBuffer;

pub struct VulkanCommandPool
{
	raw: vk::CommandPool,
	command_buffers: Vec<VulkanCommandBuffer>,
	index: usize,
}

impl VulkanDevice
{
	pub fn create_command_pool(&self, queue_type: QueueType) -> VulkanCommandPool
	{
		tracy::span!();
		let queue_index = match queue_type
		{
			QueueType::GRAPHICS => self.get_queue_family_indices().graphics_family,
			QueueType::COMPUTE => self.get_queue_family_indices().compute_family,
		};
		let raw = unsafe {
			self.raw
				.create_command_pool(
					&vk::CommandPoolCreateInfo::builder().queue_family_index(queue_index),
					None,
				)
				.unwrap()
		};

		VulkanCommandPool {
			raw,
			command_buffers: vec![],
			index: 0,
		}
	}

	pub fn destroy_command_pool(&self, command_pool: VulkanCommandPool)
	{
		tracy::span!();
		unsafe { self.raw.destroy_command_pool(command_pool.raw, None) }
	}
}

impl VulkanCommandPool
{
	pub fn begin_command_buffer(&mut self, device: &VulkanDevice) -> VulkanCommandBuffer
	{
		tracy::span!();
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
		tracy::span!();
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
		tracy::span!();
		unsafe {
			device
				.raw
				.reset_command_pool(self.raw, vk::CommandPoolResetFlags::RELEASE_RESOURCES)
				.expect("Failed to recycle command pool!");
		}
		self.index = 0;
	}

	fn expand(&mut self, device: &VulkanDevice)
	{
		tracy::span!();
		let new_cmd_buffer = *unsafe {
			device
				.raw
				.allocate_command_buffers(
					&vk::CommandBufferAllocateInfo::builder()
						.command_pool(self.raw)
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
