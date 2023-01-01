use crate::renderer::DescriptorSetInfo;
pub trait CBuffer<const S: usize> {
	fn size() -> usize;
	fn as_buffer(&self) -> [u8; S];
}

pub trait StructuredBuffer<const S: usize>: Sized {
	fn size() -> usize;
	fn copy_to_raw(src: &[Self], dst: &mut [u8]);
}

pub trait Descriptor {
	fn info() -> DescriptorSetInfo;
}
