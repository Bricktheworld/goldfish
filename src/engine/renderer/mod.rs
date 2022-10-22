pub mod backends;

use backends::vulkan::VulkanGraphicsDevice;
pub type GraphicsDevice = VulkanGraphicsDevice;
