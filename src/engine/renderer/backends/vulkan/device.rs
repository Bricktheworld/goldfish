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
use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;

pub struct VulkanGraphicsDevice
{
	entry: Entry,
	instance: ash::Instance,
	device: ash::Device,
	physical_device: vk::PhysicalDevice,
	debug_utils_loader: DebugUtils,
	debug_callback: vk::DebugUtilsMessengerEXT,
	surface: vk::SurfaceKHR,
	surface_loader: Surface,
}

unsafe extern "system" fn vulkan_debug_callback(
	message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
	message_type: vk::DebugUtilsMessageTypeFlagsEXT,
	p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
	_user_data: *mut std::os::raw::c_void,
) -> vk::Bool32
{
	use std::borrow::Cow;
	let callback_data = *p_callback_data;
	let message_id_number: i32 = callback_data.message_id_number as i32;

	let message_id_name = if callback_data.p_message_id_name.is_null()
	{
		Cow::from("")
	}
	else
	{
		CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
	};

	let message = if callback_data.p_message.is_null()
	{
		Cow::from("")
	}
	else
	{
		CStr::from_ptr(callback_data.p_message).to_string_lossy()
	};

	println!(
		"{:?}:\n{:?} [{} ({})] : {}\n",
		message_severity,
		message_type,
		message_id_name,
		&message_id_number.to_string(),
		message,
	);

	vk::FALSE
}

struct QueueFamilyIndices
{
	graphics_family: u32,
	compute_family: u32,
	present_family: u32,
}

impl VulkanGraphicsDevice
{
	pub fn new(window: &Window) -> Self
	{
		unsafe {
			let entry = Entry::linked();

			let mut extension_names = ash_window::enumerate_required_extensions(window.get_winit())
				.expect("Failed to get required extensions!")
				.to_vec();
			extension_names.push(DebugUtils::name().as_ptr());

			let layer_names = [CStr::from_bytes_with_nul_unchecked(
				b"VK_LAYER_KHRONOS_validation\0",
			)];

			let layer_names_raw: Vec<*const c_char> = layer_names
				.iter()
				.map(|raw_name| raw_name.as_ptr())
				.collect();

			let app_name = CStr::from_bytes_with_nul_unchecked(window.get_name().as_bytes());
			let app_info = vk::ApplicationInfo::builder()
				.application_name(app_name)
				.application_version(0)
				.engine_name(app_name)
				.engine_version(0)
				.api_version(vk::make_api_version(0, 1, 0, 0));

			let create_info = vk::InstanceCreateInfo::builder()
				.application_info(&app_info)
				.enabled_layer_names(&layer_names_raw)
				.enabled_extension_names(&extension_names)
				.flags(vk::InstanceCreateFlags::default());

			let instance = entry
				.create_instance(&create_info, None)
				.expect("Failed to create Vulkan instance!");

			let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
				.message_severity(
					vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
						| vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
						| vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
				)
				.message_type(
					vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
						| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
						| vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
				)
				.pfn_user_callback(Some(vulkan_debug_callback));

			let debug_utils_loader = DebugUtils::new(&entry, &instance);
			let debug_callback = debug_utils_loader
				.create_debug_utils_messenger(&debug_info, None)
				.expect("Failed to create debug messenger!");

			let surface = ash_window::create_surface(&entry, &instance, window.get_winit(), None)
				.expect("Failed to create surface!");

			let surface_loader = Surface::new(&entry, &instance);

			let find_queue_families = |dev: &vk::PhysicalDevice| -> Option<QueueFamilyIndices> {
				let properties = instance.get_physical_device_queue_family_properties(*dev);

				let mut graphics_family: Option<u32> = None;
				let mut compute_family: Option<u32> = None;
				let mut present_family: Option<u32> = None;

				for (i, prop) in properties.iter().enumerate()
				{
					if prop
						.queue_flags
						.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
					{
						graphics_family = Some(i as u32);
						compute_family = Some(i as u32);
					}
					else if prop.queue_flags.contains(vk::QueueFlags::COMPUTE)
					{
						compute_family = Some(i as u32);
					}

					if surface_loader
						.get_physical_device_surface_support(*dev, i as u32, surface)
						.unwrap_or(false)
					{
						present_family = Some(i as u32);
					}

					if let (Some(graphics_family), Some(compute_family), Some(present_family)) =
						(graphics_family, compute_family, present_family)
					{
						return Some(QueueFamilyIndices {
							graphics_family,
							compute_family,
							present_family,
						});
					}
				}

				None
			};

			let rate_device_suitability = |dev: &vk::PhysicalDevice| -> u32 {
				match find_queue_families(dev)
				{
					Some(_) =>
					{
						// TODO(Brandon): Add check for device extension support.
						// TODO(Brandon): Add swapchain support check.

						let mut score = 0;
						let properties = instance.get_physical_device_properties(*dev);
						score += match properties.device_type
						{
							vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
							vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
							_ => 0,
						};

						score += properties.limits.max_image_dimension2_d;

						return score;
					}
					None => 0,
				}
			};

			let physical_devices = instance
				.enumerate_physical_devices()
				.expect("Failed to get physical devices!");

			if physical_devices.len() == 0
			{
				panic!("No GPUs on this machine support Vulkan!");
			}

			let mut best_score = 0;
			let mut best_dev: Option<vk::PhysicalDevice> = None;
			for dev in physical_devices
			{
				let score = rate_device_suitability(&dev);
				if score > best_score
				{
					best_score = score;
					best_dev = Some(dev);
				}
			}

			let physical_device = best_dev.expect("No GPUs on this machine are supported!");

			let queue_family_indices = find_queue_families(&physical_device).expect("Failed to get queue family indices from physical device chosen. This shouldn't ever happen!");

			let mut queue_indices = HashSet::with_capacity(3);
			queue_indices.insert(queue_family_indices.graphics_family);
			queue_indices.insert(queue_family_indices.compute_family);
			queue_indices.insert(queue_family_indices.present_family);

			let queue_priorities = [1.0];
			let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = queue_indices
				.iter()
				.map(|index| {
					vk::DeviceQueueCreateInfo::builder()
						.queue_family_index(*index)
						.queue_priorities(&queue_priorities)
						.build()
				})
				.collect();

			let device_extension_names_raw = [Swapchain::name().as_ptr()];
			let features = vk::PhysicalDeviceFeatures {
				shader_clip_distance: 1,
				..Default::default()
			};

			let device_create_info = vk::DeviceCreateInfo::builder()
				.queue_create_infos(&queue_create_infos)
				.enabled_layer_names(&layer_names_raw)
				.enabled_extension_names(&device_extension_names_raw)
				.enabled_features(&features);

			let device = instance
				.create_device(physical_device, &device_create_info, None)
				.expect("Failed to create logical device!");

			Self {
				entry,
				instance,
				device,
				debug_callback,
				surface_loader,
				physical_device,
				surface,
				debug_utils_loader,
			}
		}
	}
}

impl Drop for VulkanGraphicsDevice
{
	fn drop(&mut self)
	{
		unsafe {
			self.device.device_wait_idle().expect("Wait idle failed!");

			self.device.destroy_device(None);
			self.surface_loader.destroy_surface(self.surface, None);
			self.debug_utils_loader
				.destroy_debug_utils_messenger(self.debug_callback, None);
			self.instance.destroy_instance(None);
		}
	}
}
