use crate::window::Window;
use raw_window_handle::{
	HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use ash::{
	extensions::{
		ext::DebugUtils,
		khr::{Surface, Swapchain},
	},
	vk, Entry,
};
use std::ffi::CStr;
use std::os::raw::c_char;

pub struct VulkanGraphicsDevice
{
	entry: Entry,
	instance: ash::Instance,
}

impl VulkanGraphicsDevice
{
	pub fn new(window: &Window) -> Self
	{
		let entry = Entry::linked();

		let mut extension_names = ash_window::enumerate_required_extensions(window.get_winit())
			.unwrap()
			.to_vec();
		extension_names.push(DebugUtils::name().as_ptr());

		let layer_names =
			[unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }];

		let layers_names_raw: Vec<*const c_char> = layer_names
			.iter()
			.map(|raw_name| raw_name.as_ptr())
			.collect();

		let app_name = unsafe { CStr::from_bytes_with_nul_unchecked(window.get_name().as_bytes()) };
		let app_info = vk::ApplicationInfo::builder()
			.application_name(app_name)
			.application_version(0)
			.engine_name(app_name)
			.engine_version(0)
			.api_version(vk::make_api_version(0, 1, 0, 0));

		let create_info = vk::InstanceCreateInfo::builder()
			.application_info(&app_info)
			.enabled_layer_names(&layers_names_raw)
			.enabled_extension_names(&extension_names)
			.flags(vk::InstanceCreateFlags::default());

		let instance = unsafe {
			entry
				.create_instance(&create_info, None)
				.expect("Failed to create Vulkan instance!")
		};

		Self { entry, instance }
	}
}

impl Drop for VulkanGraphicsDevice
{
	fn drop(&mut self)
	{
		unsafe { self.instance.destroy_instance(None) };
	}
}
