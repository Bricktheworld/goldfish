use super::{
	device::{VulkanDestructor, VulkanDevice, VulkanUploadContext},
	VulkanGraphicsContext, VulkanRasterCmd,
};
use crate::renderer::BufferUsage;
use ash::vk;
use gpu_allocator::vulkan as vma;
use gpu_allocator::MemoryLocation;

use std::hash::{Hash, Hasher};

impl From<BufferUsage> for vk::BufferUsageFlags {
	fn from(usage: BufferUsage) -> vk::BufferUsageFlags {
		let mut flags = vk::BufferUsageFlags::default();

		if usage.contains(BufferUsage::TransferSrc) {
			flags |= vk::BufferUsageFlags::TRANSFER_SRC;
		}

		if usage.contains(BufferUsage::TransferDst) {
			flags |= vk::BufferUsageFlags::TRANSFER_DST;
		}

		if usage.contains(BufferUsage::TransferDst) {
			flags |= vk::BufferUsageFlags::TRANSFER_DST;
		}

		if usage.contains(BufferUsage::UniformTexelBuffer) {
			flags |= vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER;
		}

		if usage.contains(BufferUsage::StorageTexelBuffer) {
			flags |= vk::BufferUsageFlags::STORAGE_TEXEL_BUFFER;
		}

		if usage.contains(BufferUsage::UniformBuffer) {
			flags |= vk::BufferUsageFlags::UNIFORM_BUFFER;
		}

		if usage.contains(BufferUsage::StorageBuffer) {
			flags |= vk::BufferUsageFlags::STORAGE_BUFFER;
		}

		if usage.contains(BufferUsage::IndexBuffer) {
			flags |= vk::BufferUsageFlags::INDEX_BUFFER;
		}

		if usage.contains(BufferUsage::VertexBuffer) {
			flags |= vk::BufferUsageFlags::VERTEX_BUFFER;
		}

		return flags;
	}
}

pub struct VulkanBuffer {
	pub raw: vk::Buffer,
	pub allocation: vma::Allocation,
	pub location: MemoryLocation,
	pub usage: BufferUsage,
	pub size: usize,
}

impl Hash for VulkanBuffer {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.raw.hash(state);
	}
}

impl PartialEq for VulkanBuffer {
	fn eq(&self, other: &Self) -> bool {
		self.raw == other.raw
	}
}

impl Eq for VulkanBuffer {}

impl VulkanUploadContext {
	pub fn create_buffer(
		&mut self,
		size: usize,
		location: MemoryLocation,
		mut usage: BufferUsage,
		alignment: Option<u64>,
		data: Option<&[u8]>,
	) -> VulkanBuffer {
		if data.is_some() {
			usage |= BufferUsage::TransferDst;
		}

		let buffer = self
			.device
			.create_empty_buffer(size, location, usage, alignment);

		if let Some(data) = data {
			let mut copy_buffer = self.device.create_empty_buffer(
				size,
				MemoryLocation::CpuToGpu,
				BufferUsage::TransferSrc,
				None,
			);

			copy_buffer.allocation.mapped_slice_mut().unwrap()[0..data.len()].copy_from_slice(data);

			self.wait_submit(|device, cmd| unsafe {
				device.cmd_copy_buffer(
					cmd,
					copy_buffer.raw,
					buffer.raw,
					&[vk::BufferCopy::builder().size(size as u64).build()],
				)
			});

			self.destroy_buffer(copy_buffer);
		}

		return buffer;
	}

	pub fn destroy_buffer(&mut self, buffer: VulkanBuffer) {
		self.device.destroy_buffer(buffer);
	}
}

impl VulkanDevice {
	pub fn create_empty_buffer(
		&self,
		mut size: usize,
		location: MemoryLocation,
		usage: BufferUsage,
		alignment: Option<u64>,
	) -> VulkanBuffer {
		if usage.contains(BufferUsage::UniformBuffer)
			|| usage.contains(BufferUsage::UniformTexelBuffer)
		{
			size = self.pad_size(size as u64) as usize;
		}

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

		if let Some(alignment) = alignment {
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

	pub fn update_buffer(&self, buffer: &mut VulkanBuffer, data: &[u8]) -> bool {
		if buffer.location != MemoryLocation::CpuToGpu {
			panic!("Cannot update buffer that is not CpuToGpu!");
		}

		if data.len() > buffer.size {
			panic!("Cannot update buffer with data that is too long!");
		}

		let dst = &mut buffer
			.allocation
			.mapped_slice_mut()
			.expect("Failed to map allocation!")[0..data.len()];

		if dst != data {
			dst.copy_from_slice(data);
			return true;
		}

		return false;
	}

	pub fn destroy_buffer(&mut self, buffer: VulkanBuffer) {
		self.queue_destruction(&mut [
			VulkanDestructor::Buffer(buffer.raw),
			VulkanDestructor::Allocation(buffer.allocation),
		])
	}
}

impl VulkanGraphicsContext {
	pub fn bind_vertex_buffer(&self, buffer: &VulkanBuffer) {
		self.queue_raster_cmd(VulkanRasterCmd::BindVertexBuffer(0, buffer.raw, 0));
	}

	pub fn bind_index_buffer(&self, buffer: &VulkanBuffer) {
		self.queue_raster_cmd(VulkanRasterCmd::BindIndexBuffer(
			buffer.raw,
			0,
			vk::IndexType::UINT16,
		));
	}
}
// impl Vulkan
