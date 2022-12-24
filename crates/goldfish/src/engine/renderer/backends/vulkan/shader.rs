use super::device::{VulkanDestructor, VulkanDevice};
use ash::vk;

pub struct VulkanShader {
	pub module: vk::ShaderModule,
}

impl VulkanDevice {
	pub fn create_shader(&self, data: &[u8]) -> VulkanShader {
		self.create_shader_with_code(
			&data
				.chunks_exact(4)
				.map(|bytes| u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
				.collect::<Vec<_>>(),
		)
	}

	pub fn create_shader_with_code(&self, code: &[u32]) -> VulkanShader {
		let module = unsafe {
			self.raw
				.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(code), None)
				.expect("Failed to create shader!")
		};

		VulkanShader { module }
	}

	pub fn destroy_shader(&mut self, shader: VulkanShader) {
		self.queue_destruction(&mut [VulkanDestructor::Shader(shader.module)]);
	}
}
