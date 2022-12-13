use crate::renderer::DescriptorSetInfo;
pub trait UniformBuffer {
	fn size(&self);
	fn write_buffer(&self, output: &mut [u8]);
}

pub trait DescriptorInfo {
	fn get() -> DescriptorSetInfo;
}
