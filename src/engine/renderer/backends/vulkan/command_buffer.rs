use ash::vk;

pub struct VulkanCommandBuffer
{
	command_buffer: vk::CommandBuffer,
}

impl VulkanCommandBuffer
{
	pub fn new(command_buffer: vk::CommandBuffer) -> Self
	{
		Self { command_buffer }
	}

	pub fn get(&self) -> vk::CommandBuffer
	{
		self.command_buffer
	}
}
