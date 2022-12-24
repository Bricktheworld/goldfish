use crate::renderer::DescriptorSetInfo;
pub trait UniformBuffer<const S: usize> {
	fn size() -> usize;
	fn as_buffer(&self) -> [u8; S];
}

pub trait Descriptor {
	fn info() -> DescriptorSetInfo;
}
