use ash::{vk, Device};
use tracy_client_sys::*;

pub struct TracyVkContext {}

impl TracyVkContext {
	pub fn new(
		physical_dev: vk::PhysicalDevice,
		device: vk::Device,
		queue: vk::Queue,
		command_buffer: vk::CommandBuffer,
		vk_get_physical_device_calibrateable_time_domains: Option<vk::PFN_vkGetPhysicalDeviceCalibrateableTimeDomainsEXT>,
		vk_get_calibrated_timestamps: Option<vk::PFN_vkGetCalibratedTimestampsEXT>,
	) -> Self {
		unsafe {
			match (vk_get_physical_device_calibrateable_time_domains, vk_get_calibrated_timestamps) {
				(Some(vk_get_physical_device_calibrateable_time_domains), Some(vk_get_calibrated_timestamps)) => {
					let mut num: u32 = 0;
					vk_get_physical_device_calibrateable_time_domains(physical_dev, &mut num as *mut u32, std::ptr::null_mut())
						.result()
						.unwrap();

					num = num.min(4);

					let mut data: [vk::TimeDomainEXT; 4] = Default::default();
					vk_get_physical_device_calibrateable_time_domains(physical_dev, &mut num as *mut u32, &mut data as *mut vk::TimeDomainEXT)
						.result()
						.unwrap();
					// let supported_domain = vk::TimeDomainEXT::TIME
				}
				_ => (),
			}
		}
		// unsafe {
		// 	___tracy_emit_gpu_new_context_serial();
		// 	Self {}
		// }
		todo!()
	}
}
