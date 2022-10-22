pub mod backends;

use backends::vulkan::{VulkanGraphicsContext, VulkanGraphicsDevice};
pub type GraphicsDevice = VulkanGraphicsDevice;
pub type GraphicsContext = VulkanGraphicsContext;
