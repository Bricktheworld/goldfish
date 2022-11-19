use super::device::{VulkanDevice, VulkanDeviceChild, VulkanUploadContext};
use crate::renderer::BufferUsage;
use ash::vk;
use gpu_allocator::vulkan as vma;
use gpu_allocator::MemoryLocation;

impl From<BufferUsage> for vk::BufferUsageFlags
{
	fn from(usage: BufferUsage) -> vk::BufferUsageFlags
	{
		let mut flags = vk::BufferUsageFlags::default();

		if usage.contains(BufferUsage::TransferSrc)
		{
			flags |= vk::BufferUsageFlags::TRANSFER_SRC;
		}

		if usage.contains(BufferUsage::TransferDst)
		{
			flags |= vk::BufferUsageFlags::TRANSFER_DST;
		}

		if usage.contains(BufferUsage::TransferDst)
		{
			flags |= vk::BufferUsageFlags::TRANSFER_DST;
		}

		if usage.contains(BufferUsage::UniformTexelBuffer)
		{
			flags |= vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER;
		}

		if usage.contains(BufferUsage::StorageTexelBuffer)
		{
			flags |= vk::BufferUsageFlags::STORAGE_TEXEL_BUFFER;
		}

		if usage.contains(BufferUsage::UniformBuffer)
		{
			flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
		}

		if usage.contains(BufferUsage::StorageBuffer)
		{
			flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
		}

		if usage.contains(BufferUsage::IndexBuffer)
		{
			flags |= vk::BufferUsageFlags::INDEX_BUFFER;
		}

		if usage.contains(BufferUsage::VertexBuffer)
		{
			flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
		}

		return flags;
	}
}

pub struct VulkanBuffer
{
	pub raw: vk::Buffer,
	pub allocation: vma::Allocation,
	pub location: MemoryLocation,
	pub usage: BufferUsage,
	pub size: usize,
}

impl VulkanDevice
{
	fn create_buffer_impl(
		&self,
		size: usize,
		location: MemoryLocation,
		usage: BufferUsage,
		alignment: Option<u64>,
	) -> VulkanBuffer
	{
		let raw = unsafe {
			self.raw
				.create_buffer(
					&vk::BufferCreateInfo::builder()
						.size(size as u64)
						.usage(usage.into())
						.sharing_mode(vk::SharingMode::EXCLUSIVE),
					None,
				)
				.expect("Failed to create buffer!")
		};

		let mut requirements = unsafe { self.raw.get_buffer_memory_requirements(raw) };

		if let Some(alignment) = alignment
		{
			requirements.alignment = requirements.alignment.max(alignment);
		}

		let mut guard = self.vma.lock().unwrap();
		let vma = guard.as_mut().unwrap();
		let allocation = vma
			.allocate(&vma::AllocationCreateDesc {
				name: "buffer",
				requirements,
				location,
				linear: true,
			})
			.expect("Failed to allocate buffer!");

		unsafe {
			self.raw
				.bind_buffer_memory(raw, allocation.memory(), allocation.offset())
				.expect("Failed to bind buffer memory!");
		}

		VulkanBuffer {
			raw,
			allocation,
			location,
			usage,
			size,
		}
	}

	pub fn create_buffer(
		&self,
		upload_context: &mut VulkanUploadContext,
		size: usize,
		location: MemoryLocation,
		mut usage: BufferUsage,
		alignment: Option<u64>,
		data: Option<&[u8]>,
	) -> VulkanBuffer
	{
		if data.is_some()
		{
			usage |= BufferUsage::TransferDst;
		}

		let buffer = self.create_buffer_impl(size, location, usage, alignment);

		if let Some(data) = data
		{
			let mut copy_buffer = self.create_buffer_impl(
				size,
				MemoryLocation::CpuToGpu,
				BufferUsage::TransferSrc,
				None,
			);

			copy_buffer.allocation.mapped_slice_mut().unwrap()[0..data.len()].copy_from_slice(data);

			upload_context.wait_submit(|device, cmd| unsafe {
				device.cmd_copy_buffer(
					cmd,
					copy_buffer.raw,
					buffer.raw,
					&[vk::BufferCopy::builder().size(size as u64).build()],
				)
			});

			copy_buffer.destroy(&self);
		}

		return buffer;
	}
}

impl VulkanDeviceChild for VulkanBuffer
{
	fn destroy(self, device: &VulkanDevice)
	{
		unsafe {
			device.raw.destroy_buffer(self.raw, None);

			let mut guard = device.vma.lock().unwrap();
			let vma = guard.as_mut().unwrap();

			vma.free(self.allocation)
				.expect("Failed to free allocation!");
		}
	}
}
