pub mod backends;

use backends::vulkan::device::VulkanGraphicsDevice;
pub type GraphicsDevice = VulkanGraphicsDevice;
