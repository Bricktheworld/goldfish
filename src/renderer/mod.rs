pub mod backends;
use super::window::Window;
use backends::vulkan::device::VulkanGraphicsDevice;
use backends::Backend;

pub struct GraphicsDevice
{
	device: Backend,
}

impl GraphicsDevice
{
	pub fn new(window: &Window) -> Self
	{
		let device = VulkanGraphicsDevice::new(window);
		Self {
			device: Backend::Vulkan(device),
		}
	}
}
