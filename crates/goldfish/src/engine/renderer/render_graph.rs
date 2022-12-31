use super::*;
use std::collections::HashSet;

#[derive(Debug, Clone)]
enum PassCmd {
	BeginRenderPass {
		render_pass: GraphRenderPassHandle,
		clear_values: Vec<ClearValue>,
	},
	EndRenderPass {},
	BindRasterPipeline {
		pipeline: GraphRasterPipelineHandle,
	},
	BindComputePipeline {
		pipeline: GraphComputePipelineHandle,
	},
	BindDescriptor {
		set: u32,
		descriptor: GraphDescriptorHandle,
		pipeline: GraphPipelineHandle,
	},
	DrawMesh {
		mesh: GraphImportedMeshHandle,
	},
	Draw {
		vertex_count: u32,
		instance_count: u32,
		first_vertex: u32,
		first_instance: u32,
	},
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct BufferCacheKey {
	size: usize,
	usage: BufferUsage,
	location: MemoryLocation,
}

#[derive(Default)]
struct BufferCache {
	buffers: Vec<GpuBuffer>,
	cache: HashMap<BufferCacheKey, Vec<usize>>,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct AttachmentCacheKey {
	width: u32,
	height: u32,
	format: TextureFormat,
	usage: TextureUsage,
}

#[derive(Default)]
struct AttachmentCache {
	attachments: Vec<Texture>,
	cache: HashMap<AttachmentCacheKey, Vec<usize>>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct FramebufferCacheKey {
	width: u32,
	height: u32,
	attachments: Vec<usize>,
	render_pass: usize,
}

#[derive(Default)]
struct FramebufferCache {
	framebuffers: Vec<Framebuffer>,
	cache: HashMap<FramebufferCacheKey, usize>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RenderPassCacheKey {
	color_attachment_descs: Vec<AttachmentDescription>,
	depth_attachment_desc: Option<AttachmentDescription>,
}

#[derive(Default)]
struct RenderPassCache {
	render_passes: Vec<RenderPass>,
	cache: HashMap<RenderPassCacheKey, usize>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RasterPipelineCacheKey {
	vs: ash::vk::ShaderModule,         // TODO(Brandon): Make this platform agnostic or find some better way to do this.
	ps: Option<ash::vk::ShaderModule>, // This applies to all borrowed resources where we need some hashable way of identifying them.
	descriptor_layouts: Vec<DescriptorLayout>,
	render_pass: usize,
	depth_compare_op: Option<DepthCompareOp>,
	depth_write: bool,
	face_cull: FaceCullMode,
	push_constant_bytes: usize,
	vertex_input_info: VertexInputInfo,
	polygon_mode: PolygonMode,
}

#[derive(Default)]
struct RasterPipelineCache {
	pipelines: Vec<Pipeline>,
	cache: HashMap<RasterPipelineCacheKey, usize>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ComputePipelineCacheKey {
	cs: ash::vk::ShaderModule, // TODO(Brandon): Same thing as raster pipeline cache key
	descriptor_layouts: Vec<DescriptorLayout>,
}

#[derive(Default)]
struct ComputePipelineCache {
	pipelines: Vec<Pipeline>,
	cache: HashMap<ComputePipelineCacheKey, usize>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum DescriptorHeapCacheKeyBinding {
	ImportedBuffer {
		buffer: ash::vk::Buffer,
	}, // TODO(Brandon): Same thing as raster pipeline cache key. In general for imported resources
	ImportedTexture {
		image: ash::vk::Image,
		sampler: ash::vk::Sampler,
		image_view: ash::vk::ImageView,
	}, // We need a better way of identifying them.
	Buffer {
		buffer: usize,
	},
	Attachment {
		attachment: usize,
	},
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct DescriptorHeapCacheKey {
	bindings: Vec<(u32, DescriptorHeapCacheKeyBinding)>,
}

struct DescriptorHeapCache {
	heap: DescriptorHeap,
	cache: HashMap<DescriptorHeapCacheKey, DescriptorHandle>,
}

#[derive(Default)]
pub struct RenderGraphCache {
	buffer_cache: BufferCache,
	attachment_cache: AttachmentCache,
	framebuffer_cache: FramebufferCache,
	render_pass_cache: RenderPassCache,
	raster_pipeline_cache: RasterPipelineCache,
	compute_pipeline_cache: ComputePipelineCache,
	descriptor_layout_cache: DescriptorLayoutCache,
	descriptor_heap_caches: HashMap<*const DescriptorSetInfo, DescriptorHeapCache>,
}

impl RenderGraphCache {
	fn alloc_render_pass(&mut self, graphics_device: &GraphicsDevice, key: &RenderPassCacheKey) -> usize {
		*self.render_pass_cache.cache.entry(key.clone()).or_insert_with(|| {
			println!("Allocated render pass! {:?}", key);
			self.render_pass_cache
				.render_passes
				.push(graphics_device.create_render_pass(&key.color_attachment_descs, key.depth_attachment_desc));

			self.render_pass_cache.render_passes.len() - 1
		})
	}

	fn get_render_pass_index(&self, key: &RenderPassCacheKey) -> usize {
		*self.render_pass_cache.cache.get(key).unwrap()
	}

	fn get_render_pass(&self, key: &RenderPassCacheKey) -> &RenderPass {
		&self.render_pass_cache.render_passes[self.get_render_pass_index(key)]
	}

	fn alloc_framebuffer(&mut self, graphics_device: &GraphicsDevice, key: &FramebufferCacheKey) -> usize {
		*self.framebuffer_cache.cache.entry(key.clone()).or_insert_with(|| {
			println!("Allocated framebuffer!");

			let render_pass = &self.render_pass_cache.render_passes[key.render_pass];
			let attachments = key.attachments.iter().map(|a| &self.attachment_cache.attachments[*a]).collect::<Vec<_>>();

			self.framebuffer_cache
				.framebuffers
				.push(graphics_device.create_framebuffer(key.width, key.height, render_pass, &attachments));

			self.framebuffer_cache.framebuffers.len() - 1
		})
	}

	fn get_framebuffer_index(&self, key: &FramebufferCacheKey) -> usize {
		*self.framebuffer_cache.cache.get(key).unwrap()
	}

	fn get_framebuffer(&self, key: &FramebufferCacheKey) -> &Framebuffer {
		&self.framebuffer_cache.framebuffers[self.get_framebuffer_index(key)]
	}

	fn alloc_raster_pipeline(&mut self, graphics_context: &mut GraphicsContext, graphics_device: &GraphicsDevice, key: &RasterPipelineCacheKey) -> usize {
		*self.raster_pipeline_cache.cache.entry(key.clone()).or_insert_with(|| {
			println!("Allocated pipeline!");
			// TODO(Brandon): Kinda a messy hack to get around rust, not sure if there is a better way to do this...
			let ps = Shader {
				module: key.ps.unwrap_or(ash::vk::ShaderModule::null()),
			};
			let ps = key.ps.map_or(None, |_| Some(&ps));

			// TODO(Brandon): Really good example of how we should allow for fetching of the render pass from the swapchain.
			self.raster_pipeline_cache.pipelines.push(if key.render_pass == usize::MAX {
				graphics_context.create_raster_pipeline(
					&Shader { module: key.vs },
					ps,
					&key.descriptor_layouts,
					key.depth_compare_op,
					key.depth_write,
					key.face_cull,
					key.push_constant_bytes,
					key.vertex_input_info,
					key.polygon_mode,
				)
			} else {
				graphics_device.create_raster_pipeline(
					&Shader { module: key.vs },
					ps,
					&key.descriptor_layouts,
					&mut self.render_pass_cache.render_passes[key.render_pass],
					key.depth_compare_op,
					key.depth_write,
					key.face_cull,
					key.push_constant_bytes,
					key.vertex_input_info,
					key.polygon_mode,
				)
			});

			self.raster_pipeline_cache.pipelines.len() - 1
		})
	}

	fn get_raster_pipeline_index(&self, key: &RasterPipelineCacheKey) -> usize {
		*self.raster_pipeline_cache.cache.get(key).unwrap()
	}

	fn get_raster_pipeline(&self, key: &RasterPipelineCacheKey) -> &Pipeline {
		&self.raster_pipeline_cache.pipelines[self.get_raster_pipeline_index(key)]
	}

	fn alloc_compute_pipeline(&mut self, graphics_device: &GraphicsDevice, key: &ComputePipelineCacheKey) -> usize {
		*self.compute_pipeline_cache.cache.entry(key.clone()).or_insert_with(|| {
			println!("Allocated compute pipeline");
			self.compute_pipeline_cache
				.pipelines
				.push(graphics_device.create_compute_pipeline(&Shader { module: key.cs }, &key.descriptor_layouts));

			self.compute_pipeline_cache.pipelines.len() - 1
		})
	}

	fn get_compute_pipeline_index(&self, key: &ComputePipelineCacheKey) -> usize {
		*self.compute_pipeline_cache.cache.get(key).unwrap()
	}

	fn get_compute_pipeline(&self, key: &ComputePipelineCacheKey) -> &Pipeline {
		&self.compute_pipeline_cache.pipelines[self.get_compute_pipeline_index(key)]
	}

	fn alloc_attachments(&mut self, graphics_device: &GraphicsDevice, key: &AttachmentCacheKey, count: usize) {
		let attachments = self.attachment_cache.cache.entry(key.clone()).or_default();
		while attachments.len() < count {
			println!("Allocated attachment!");
			attachments.push(self.attachment_cache.attachments.len());
			self.attachment_cache
				.attachments
				.push(graphics_device.create_texture(key.width, key.height, key.format, key.usage | TextureUsage::ATTACHMENT));
		}
	}

	fn alloc_buffers(&mut self, graphics_device: &GraphicsDevice, key: &BufferCacheKey, count: usize) {
		let buffers = self.buffer_cache.cache.entry(*key).or_default();
		while buffers.len() < count {
			println!("Allocated buffer!");
			buffers.push(self.buffer_cache.buffers.len());
			self.buffer_cache.buffers.push(graphics_device.create_empty_buffer(key.size, key.location, key.usage, None));
		}
	}

	fn alloc_descriptor(&mut self, graphics_device: &GraphicsDevice, descriptor_info: &'static DescriptorSetInfo, key: &DescriptorHeapCacheKey) -> DescriptorHandle {
		self.register_graphics_descriptor_layout(graphics_device, descriptor_info);

		let descriptor_cache = self.descriptor_heap_caches.get_mut(&(descriptor_info as *const DescriptorSetInfo)).unwrap();

		*descriptor_cache.cache.entry(key.clone()).or_insert_with(|| {
			println!("Allocated descriptor!");
			descriptor_cache.heap.alloc().unwrap()
		})
	}

	fn get_descriptor_heap(&self, descriptor_info: &'static DescriptorSetInfo) -> &DescriptorHeap {
		&self.descriptor_heap_caches.get(&(descriptor_info as *const DescriptorSetInfo)).unwrap().heap
	}

	fn register_graphics_descriptor_layout(&mut self, graphics_device: &GraphicsDevice, descriptor_info: &'static DescriptorSetInfo) -> DescriptorLayout {
		let layout = graphics_device.get_graphics_layout(&mut self.descriptor_layout_cache, descriptor_info);

		self.descriptor_heap_caches.entry(descriptor_info).or_insert_with(|| DescriptorHeapCache {
			heap: graphics_device.create_descriptor_heap(layout),
			cache: Default::default(),
		});

		layout
	}

	fn register_compute_descriptor_layout(&mut self, graphics_device: &GraphicsDevice, descriptor_info: &'static DescriptorSetInfo) -> DescriptorLayout {
		let layout = graphics_device.get_compute_layout(&mut self.descriptor_layout_cache, descriptor_info);

		self.descriptor_heap_caches.entry(descriptor_info).or_insert_with(|| DescriptorHeapCache {
			heap: graphics_device.create_descriptor_heap(layout),
			cache: Default::default(),
		});

		layout
	}

	pub fn destroy(self, graphics_device: &mut GraphicsDevice) {
		for attachment in self.attachment_cache.attachments {
			graphics_device.destroy_texture(attachment);
		}

		for pipeline in self.raster_pipeline_cache.pipelines {
			graphics_device.destroy_pipeline(pipeline);
		}

		for render_pass in self.render_pass_cache.render_passes {
			graphics_device.destroy_render_pass(render_pass);
		}

		for framebuffer in self.framebuffer_cache.framebuffers {
			graphics_device.destroy_framebuffer(framebuffer);
		}

		for (_, cache) in self.descriptor_heap_caches {
			graphics_device.destroy_descriptor_heap(cache.heap);
		}

		graphics_device.destroy_descriptor_layout_cache(self.descriptor_layout_cache);
	}
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct AttachmentDesc {
	pub name: &'static str,
	pub width: u32,
	pub height: u32,
	pub format: TextureFormat,
	pub load_op: LoadOp,
	pub store_op: StoreOp,
	pub usage: TextureUsage,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct BufferDesc {
	pub name: &'static str,
	pub size: usize,
	pub usage: BufferUsage,
	pub location: MemoryLocation,
}

pub struct RenderPassDesc<'a, 'b> {
	pub name: &'static str,
	pub color_attachments: &'b mut [&'b mut MutableGraphAttachmentHandle],
	pub depth_attachment: Option<&'a mut MutableGraphAttachmentHandle>,
}

#[derive(Clone)]
pub struct RasterPipelineDesc<'a, 'b> {
	pub name: &'static str,
	pub vs: &'a Shader,
	pub ps: Option<&'a Shader>,
	pub descriptor_layouts: &'b [&'static DescriptorSetInfo],
	pub render_pass: GraphRenderPassHandle,
	pub depth_compare_op: Option<DepthCompareOp>,
	pub depth_write: bool,
	pub face_cull: FaceCullMode,
	pub push_constant_bytes: usize,
	pub vertex_input_info: VertexInputInfo,
	pub polygon_mode: PolygonMode,
}

pub enum DescriptorBindingDesc<'a, 'b> {
	ImportedBuffer(&'a GpuBuffer),
	ImportedTexture(&'a Texture),
	Buffer(GraphBufferHandle),
	MutableBuffer(&'b mut MutableGraphBufferHandle),
	Attachment(GraphAttachmentHandle),
	MutableAttachment(&'b mut MutableGraphAttachmentHandle),
}

pub struct DescriptorDesc<'a, 'b> {
	pub name: &'static str,
	pub descriptor_layout: &'static DescriptorSetInfo,
	pub bindings: &'b mut [(u32, DescriptorBindingDesc<'a, 'b>)],
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum GraphImportedResource<'a> {
	Shader(&'a Shader),
	Mesh(&'a Mesh),
	Buffer(&'a GpuBuffer),
	Texture(&'a Texture),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedShaderHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedBufferHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedTextureHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedMeshHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum GraphOwnedResourceDescriptorBinding {
	ImportedBuffer(GraphImportedBufferHandle),
	ImportedTexture(GraphImportedTextureHandle),
	Buffer(GraphBufferHandle),
	MutableBuffer(MutableGraphBufferHandle),
	Attachment(GraphAttachmentHandle),
	MutableAttachment(MutableGraphAttachmentHandle),
}

#[derive(Debug, Clone)]
enum GraphOwnedResource {
	RasterPipeline {
		name: &'static str,
		vs: GraphImportedShaderHandle,
		ps: Option<GraphImportedShaderHandle>,
		descriptor_layouts: Vec<&'static DescriptorSetInfo>,
		render_pass: GraphRenderPassHandle,
		depth_compare_op: Option<DepthCompareOp>,
		depth_write: bool,
		face_cull: FaceCullMode,
		push_constant_bytes: usize,
		vertex_input_info: VertexInputInfo,
		polygon_mode: PolygonMode,
	},
	ComputePipeline {
		name: &'static str,
		cs: GraphImportedShaderHandle,
		descriptor_layouts: Vec<&'static DescriptorSetInfo>,
	},
	RenderPass {
		name: &'static str,
		color_attachments: Vec<MutableGraphAttachmentHandle>,
		depth_attachment: Option<MutableGraphAttachmentHandle>,
	},
	OutputRenderPass {},
	Attachment {
		name: &'static str,
		width: u32,
		height: u32,
		format: TextureFormat,
		usage: TextureUsage,
		load_op: LoadOp,
		store_op: StoreOp,
	},
	Buffer {
		name: &'static str,
		size: usize,
		usage: BufferUsage,
		location: MemoryLocation,
	},
	DescriptorSet {
		name: &'static str,
		descriptor_layout: &'static DescriptorSetInfo,
		bindings: Vec<(u32, GraphOwnedResourceDescriptorBinding)>,
	},
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphRasterPipelineHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphComputePipelineHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum GraphPipelineHandle {
	Raster(GraphRasterPipelineHandle),
	Compute(GraphComputePipelineHandle),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphAttachmentHandle {
	id: usize,
	src_stage: ash::vk::PipelineStageFlags,
	dst_stage: ash::vk::PipelineStageFlags,
	src_access: ash::vk::AccessFlags,
	dst_access: ash::vk::AccessFlags,
	initial_layout: ImageLayout,
	final_layout: ImageLayout,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MutableGraphAttachmentHandle {
	id: usize,
	layout: ImageLayout,
	stage: ash::vk::PipelineStageFlags,
	access: ash::vk::AccessFlags,
}

impl MutableGraphAttachmentHandle {
	pub fn read(self) -> GraphAttachmentHandle {
		GraphAttachmentHandle {
			id: self.id,
			src_stage: self.stage,
			src_access: self.access,
			initial_layout: self.layout,
			// TODO(Brandon): In the future, we might need to support different configurations for read attachments. I _think_ this will be fine for now, but it's still hard-coded :/
			dst_stage: ash::vk::PipelineStageFlags::VERTEX_SHADER | ash::vk::PipelineStageFlags::FRAGMENT_SHADER | ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			dst_access: ash::vk::AccessFlags::SHADER_READ,
			final_layout: ImageLayout::ShaderReadOnlyOptimal,
		}
	}
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphBufferHandle {
	id: usize,
	src_stage: ash::vk::PipelineStageFlags,
	dst_stage: ash::vk::PipelineStageFlags,
	src_access: ash::vk::AccessFlags,
	dst_access: ash::vk::AccessFlags,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MutableGraphBufferHandle {
	id: usize,
	stage: ash::vk::PipelineStageFlags,
	access: ash::vk::AccessFlags,
}

impl MutableGraphBufferHandle {
	pub fn read(self) -> GraphBufferHandle {
		GraphBufferHandle {
			id: self.id,
			src_stage: self.stage,
			src_access: self.access,
			dst_stage: ash::vk::PipelineStageFlags::VERTEX_SHADER | ash::vk::PipelineStageFlags::FRAGMENT_SHADER | ash::vk::PipelineStageFlags::COMPUTE_SHADER,
			dst_access: ash::vk::AccessFlags::SHADER_READ,
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GraphRenderPassHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphDescriptorHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct PassHandle {
	id: usize,
}

#[derive(Debug)]
struct PassDependencyNode {
	pass: PassHandle,
	dependencies: Vec<PassDependencyNode>,
}

#[derive(Debug, Clone)]
pub struct RecordedPass {
	name: &'static str,
	pass: PassHandle,
	cmds: Vec<PassCmd>,
	read_attachments: HashSet<GraphAttachmentHandle>,
	write_attachments: HashSet<MutableGraphAttachmentHandle>,

	read_buffers: HashSet<GraphBufferHandle>,
	write_buffers: HashSet<MutableGraphBufferHandle>,
}

pub struct RenderGraph<'a> {
	passes: Vec<RecordedPass>,
	owned_resources: Vec<GraphOwnedResource>,
	resource_to_owning_pass: HashMap<usize, PassHandle>,
	imported_resources: Vec<GraphImportedResource<'a>>,
	cache: &'a mut RenderGraphCache,
}

struct VirtualToPhysicalResourceMap<T: Copy> {
	map: HashMap<usize, T>,
}

impl<T: Copy> VirtualToPhysicalResourceMap<T> {
	fn new() -> Self {
		Self { map: Default::default() }
	}
	fn map_physical(&mut self, handle: usize, physical: T) {
		self.map.entry(handle).or_insert(physical);
	}

	fn get_physical(&self, id: usize) -> T {
		self.map[&id]
	}
}

struct GraphPhysicalResourceMap {
	attachment_map: VirtualToPhysicalResourceMap<usize>,
	buffer_map: VirtualToPhysicalResourceMap<usize>,
	descriptor_map: VirtualToPhysicalResourceMap<(DescriptorHandle, &'static DescriptorSetInfo)>,
	render_pass_map: VirtualToPhysicalResourceMap<usize>,
	framebuffer_map: VirtualToPhysicalResourceMap<usize>,
	raster_pipeline_map: VirtualToPhysicalResourceMap<usize>,
	compute_pipeline_map: VirtualToPhysicalResourceMap<usize>,
}

impl GraphPhysicalResourceMap {
	fn new(graph: &mut RenderGraph, graphics_device: &mut GraphicsDevice, graphics_context: &mut GraphicsContext) -> Self {
		let attachment_map = Self::alloc_attachments(graph, graphics_device);
		let buffer_map = Self::alloc_buffers(graph, graphics_device);
		let descriptor_map = Self::alloc_descriptors(graph, graphics_device, graphics_context, &attachment_map, &buffer_map);
		let (render_pass_map, framebuffer_map) = Self::alloc_render_passes(graph, graphics_device, &attachment_map);
		let raster_pipeline_map = Self::alloc_raster_pipelines(graph, graphics_device, graphics_context, &render_pass_map);
		let compute_pipeline_map = Self::alloc_compute_pipelines(graph, graphics_device);

		Self {
			attachment_map,
			buffer_map,
			descriptor_map,
			render_pass_map,
			framebuffer_map,
			raster_pipeline_map,
			compute_pipeline_map,
		}
	}

	fn get_render_pass<'a>(&self, graph: &'a RenderGraph, render_pass: GraphRenderPassHandle) -> Option<(&'a RenderPass, &'a Framebuffer)> {
		let physical_render_pass = self.render_pass_map.get_physical(render_pass.id);
		if physical_render_pass == usize::MAX {
			return None;
		}

		let physical_framebuffer = self.framebuffer_map.get_physical(render_pass.id);
		Some((
			&graph.cache.render_pass_cache.render_passes[physical_render_pass],
			&graph.cache.framebuffer_cache.framebuffers[physical_framebuffer],
		))
	}

	fn get_raster_pipeline<'a>(&self, graph: &'a RenderGraph, pipeline: GraphRasterPipelineHandle) -> &'a Pipeline {
		let physical_pipeline = self.raster_pipeline_map.get_physical(pipeline.id);

		&graph.cache.raster_pipeline_cache.pipelines[physical_pipeline]
	}

	fn get_compute_pipeline<'a>(&self, graph: &'a RenderGraph, pipeline: GraphComputePipelineHandle) -> &'a Pipeline {
		let physical_pipeline = self.compute_pipeline_map.get_physical(pipeline.id);

		&graph.cache.compute_pipeline_cache.pipelines[physical_pipeline]
	}

	fn get_descriptor<'a>(&self, graph: &'a RenderGraph, descriptor: GraphDescriptorHandle) -> (DescriptorHandle, &'a DescriptorHeap) {
		let (descriptor, info) = self.descriptor_map.get_physical(descriptor.id);
		(descriptor, graph.cache.get_descriptor_heap(info))
	}

	fn get_attachment<'a>(&self, graph: &'a RenderGraph, attachment: GraphAttachmentHandle) -> &'a Texture {
		let physical_attachment = self.attachment_map.get_physical(attachment.id);

		&graph.cache.attachment_cache.attachments[physical_attachment]
	}

	fn get_buffer<'a>(&self, graph: &'a RenderGraph, buffer: GraphBufferHandle) -> &'a GpuBuffer {
		let physical_buffer = self.buffer_map.get_physical(buffer.id);

		&graph.cache.buffer_cache.buffers[physical_buffer]
	}

	fn alloc_attachments(graph: &mut RenderGraph, graphics_device: &mut GraphicsDevice) -> VirtualToPhysicalResourceMap<usize> {
		let mut attachment_type_to_virtual = HashMap::<AttachmentCacheKey, Vec<usize>>::new();

		for (i, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				&GraphOwnedResource::Attachment { width, height, format, usage, .. } => {
					let key = AttachmentCacheKey { width, height, format, usage };

					attachment_type_to_virtual.entry(key).or_default().push(i);
				}
				_ => {}
			}
		}

		for (key, virtual_resources) in attachment_type_to_virtual.iter() {
			graph.cache.alloc_attachments(graphics_device, key, virtual_resources.len());
		}

		let mut attachment_map = VirtualToPhysicalResourceMap::new();

		// TODO(Brandon): Optimize this by mapping virtual to physical attachments based on existing framebuffers and descriptors to reduce allocations.
		for (key, virtual_resources) in attachment_type_to_virtual {
			for (i, virtual_resource) in virtual_resources.into_iter().enumerate() {
				let index = graph.cache.attachment_cache.cache[&key][i];
				attachment_map.map_physical(virtual_resource, index);
			}
		}

		attachment_map
	}

	fn alloc_buffers(graph: &mut RenderGraph, graphics_device: &mut GraphicsDevice) -> VirtualToPhysicalResourceMap<usize> {
		let mut buffer_type_to_virtual = HashMap::<BufferCacheKey, Vec<usize>>::new();

		for (i, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				&GraphOwnedResource::Buffer { size, usage, location, .. } => {
					let key = BufferCacheKey { size, usage, location };

					buffer_type_to_virtual.entry(key).or_default().push(i);
				}
				_ => {}
			}
		}

		for (key, virtual_resources) in buffer_type_to_virtual.iter() {
			graph.cache.alloc_buffers(graphics_device, key, virtual_resources.len());
		}

		let mut buffer_map = VirtualToPhysicalResourceMap::new();

		// TODO(Brandon): Optimize this by mapping virtual to physical attachments based on existing framebuffers and descriptors to reduce allocations.
		for (key, virtual_resources) in buffer_type_to_virtual {
			for (i, virtual_resource) in virtual_resources.into_iter().enumerate() {
				let index = graph.cache.buffer_cache.cache[&key][i];
				buffer_map.map_physical(virtual_resource, index);
			}
		}

		buffer_map
	}

	fn alloc_descriptors(
		graph: &mut RenderGraph,
		graphics_device: &mut GraphicsDevice,
		graphics_context: &mut GraphicsContext,
		attachment_map: &VirtualToPhysicalResourceMap<usize>,
		buffer_map: &VirtualToPhysicalResourceMap<usize>,
	) -> VirtualToPhysicalResourceMap<(DescriptorHandle, &'static DescriptorSetInfo)> {
		let mut descriptor_map = VirtualToPhysicalResourceMap::new();
		for (id, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::DescriptorSet { descriptor_layout, bindings, .. } => {
					let key_bindings = bindings
						.iter()
						.map(|(i, binding)| {
							(
								*i,
								match binding {
									GraphOwnedResourceDescriptorBinding::ImportedBuffer(buffer) => match &graph.imported_resources[buffer.id] {
										GraphImportedResource::Buffer(buffer) => DescriptorHeapCacheKeyBinding::ImportedBuffer { buffer: buffer.raw },
										_ => unreachable!("Invalid buffer handle!"),
									},
									GraphOwnedResourceDescriptorBinding::ImportedTexture(texture) => match &graph.imported_resources[texture.id] {
										GraphImportedResource::Texture(texture) => DescriptorHeapCacheKeyBinding::ImportedTexture {
											image: texture.image,
											sampler: texture.sampler,
											image_view: texture.image_view,
										},
										_ => unreachable!("Invalid texture handle!"),
									},
									GraphOwnedResourceDescriptorBinding::Buffer(buffer) => DescriptorHeapCacheKeyBinding::Buffer {
										buffer: buffer_map.get_physical(buffer.id),
									},
									GraphOwnedResourceDescriptorBinding::MutableBuffer(buffer) => DescriptorHeapCacheKeyBinding::Buffer {
										buffer: buffer_map.get_physical(buffer.id),
									},
									GraphOwnedResourceDescriptorBinding::Attachment(attachment) => DescriptorHeapCacheKeyBinding::Attachment {
										attachment: attachment_map.get_physical(attachment.id),
									},
									GraphOwnedResourceDescriptorBinding::MutableAttachment(attachment) => DescriptorHeapCacheKeyBinding::Attachment {
										attachment: attachment_map.get_physical(attachment.id),
									},
								},
							)
						})
						.collect::<Vec<_>>();
					let key = DescriptorHeapCacheKey { bindings: key_bindings };

					let descriptor = graph.cache.alloc_descriptor(graphics_device, descriptor_layout, &key);

					descriptor_map.map_physical(id, (descriptor, *descriptor_layout));

					// Update the descriptor set with the appropriate data.
					// TODO(Brandon): We should first check to make sure that we actually need to do this before we do so to prevent unnecessary vkUpdateDescriptorSet calls.
					let descriptor_heap = &graph.cache.get_descriptor_heap(descriptor_layout);

					let buffers = bindings
						.iter()
						.filter(|(_, ty)| match ty {
							GraphOwnedResourceDescriptorBinding::ImportedBuffer(..) => true,
							_ => false,
						})
						.map(|(binding, buffer)| {
							(
								*binding,
								match buffer {
									GraphOwnedResourceDescriptorBinding::ImportedBuffer(buffer) => match graph.imported_resources[buffer.id] {
										GraphImportedResource::Buffer(buffer) => buffer,
										_ => unreachable!("Invalid imported buffer!"),
									},
									GraphOwnedResourceDescriptorBinding::Buffer(buffer) => &graph.cache.buffer_cache.buffers[buffer_map.get_physical(buffer.id)],
									_ => unreachable!(),
								},
							)
						})
						.collect::<Vec<_>>();

					let images = bindings
						.iter()
						.filter(|(_, ty)| match ty {
							GraphOwnedResourceDescriptorBinding::Attachment(..) => true,
							_ => false,
						})
						.map(|(binding, image)| match image {
							GraphOwnedResourceDescriptorBinding::Attachment(attachment) => {
								let physical_attachment = &graph.cache.attachment_cache.attachments[attachment_map.get_physical(attachment.id)];

								(*binding, physical_attachment, attachment.final_layout)
							}
							_ => unreachable!(),
						})
						.collect::<Vec<_>>();

					graphics_context.update_descriptor(&buffers, &images, descriptor_layout, descriptor_heap, &descriptor);
				}
				_ => {}
			}
		}

		descriptor_map
	}

	fn alloc_render_passes(
		graph: &mut RenderGraph,
		graphics_device: &mut GraphicsDevice,
		attachment_map: &VirtualToPhysicalResourceMap<usize>,
	) -> (VirtualToPhysicalResourceMap<usize>, VirtualToPhysicalResourceMap<usize>) {
		let mut render_pass_map = VirtualToPhysicalResourceMap::new();
		let mut framebuffer_map = VirtualToPhysicalResourceMap::new();

		for (id, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::RenderPass {
					color_attachments, depth_attachment, ..
				} => {
					let color_attachment_descs = color_attachments
						.iter()
						.map(|handle| match &graph.owned_resources[handle.id] {
							&GraphOwnedResource::Attachment { format, usage, load_op, store_op, .. } => AttachmentDescription {
								format,
								usage,
								load_op,
								store_op,
								// TODO(Brandon): In the future we might need to support other layout transitions in case we want to write to an attachment that was previously read.
								initial_layout: ImageLayout::Undefined,
								final_layout: handle.layout,
							},
							_ => unreachable!(),
						})
						.collect::<Vec<_>>();

					let depth_attachment_desc = depth_attachment.map_or(None, |handle| match &graph.owned_resources[handle.id] {
						&GraphOwnedResource::Attachment { format, usage, load_op, store_op, .. } => Some(AttachmentDescription {
							format,
							usage,
							load_op,
							store_op,
							initial_layout: ImageLayout::Undefined,
							final_layout: handle.layout,
						}),
						_ => unreachable!(),
					});

					let render_pass_key = RenderPassCacheKey {
						color_attachment_descs,
						depth_attachment_desc,
					};

					let render_pass = graph.cache.alloc_render_pass(graphics_device, &render_pass_key);

					let width = color_attachments
						.iter()
						.map(|a| match &graph.owned_resources[a.id] {
							&GraphOwnedResource::Attachment { width, .. } => width,
							_ => unreachable!(),
						})
						.chain(depth_attachment.into_iter().map(|d| match &graph.owned_resources[d.id] {
							&GraphOwnedResource::Attachment { width, .. } => width,
							_ => unreachable!(),
						}))
						.min()
						.unwrap_or(0);

					let height = color_attachments
						.iter()
						.map(|a| match &graph.owned_resources[a.id] {
							&GraphOwnedResource::Attachment { height, .. } => height,
							_ => unreachable!(),
						})
						.chain(depth_attachment.into_iter().map(|d| match &graph.owned_resources[d.id] {
							&GraphOwnedResource::Attachment { height, .. } => height,
							_ => unreachable!(),
						}))
						.min()
						.unwrap_or(0);

					let mut attachments = color_attachments.iter().map(|a| attachment_map.get_physical(a.id)).collect::<Vec<_>>();
					if let Some(a) = depth_attachment {
						attachments.push(attachment_map.get_physical(a.id));
					}

					let framebuffer_key = FramebufferCacheKey {
						width,
						height,
						attachments,
						render_pass,
					};

					let framebuffer = graph.cache.alloc_framebuffer(graphics_device, &framebuffer_key);

					// NOTE(Brandon): Framebuffer and render pass resources are internally bound on the same virtual index.
					render_pass_map.map_physical(id, render_pass);
					framebuffer_map.map_physical(id, framebuffer);
				}
				GraphOwnedResource::OutputRenderPass {} => {
					render_pass_map.map_physical(id, usize::MAX);
				}
				_ => {}
			}
		}

		(render_pass_map, framebuffer_map)
	}

	fn alloc_raster_pipelines(
		graph: &mut RenderGraph,
		graphics_device: &mut GraphicsDevice,
		graphics_context: &mut GraphicsContext,
		render_pass_map: &VirtualToPhysicalResourceMap<usize>,
	) -> VirtualToPhysicalResourceMap<usize> {
		let mut pipeline_map = VirtualToPhysicalResourceMap::new();

		for (id, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::RasterPipeline {
					vs,
					ps,
					descriptor_layouts,
					render_pass,
					depth_compare_op,
					depth_write,
					face_cull,
					push_constant_bytes,
					vertex_input_info,
					polygon_mode,
					..
				} => {
					// TODO(Brandon): Definitely don't do it like this, this is a hack to get the raw pointer
					let vs = match &graph.imported_resources[vs.id] {
						GraphImportedResource::Shader(shader) => shader.module,
						_ => panic!("Invalid vertex shader handle!"),
					};

					let ps = if let Some(ps) = ps {
						match &graph.imported_resources[ps.id] {
							GraphImportedResource::Shader(shader) => Some(shader.module),
							_ => panic!("Invalid vertex shader handle!"),
						}
					} else {
						None
					};

					let descriptor_layouts = descriptor_layouts
						.into_iter()
						.map(|info| graph.cache.register_graphics_descriptor_layout(graphics_device, info))
						.collect::<Vec<_>>();

					let render_pass = render_pass_map.get_physical(render_pass.id);

					let key = RasterPipelineCacheKey {
						vs,
						ps,
						render_pass,
						descriptor_layouts,
						depth_compare_op: *depth_compare_op,
						depth_write: *depth_write,
						face_cull: *face_cull,
						push_constant_bytes: *push_constant_bytes,
						vertex_input_info: *vertex_input_info,
						polygon_mode: *polygon_mode,
					};

					let pipeline = graph.cache.alloc_raster_pipeline(graphics_context, graphics_device, &key);
					pipeline_map.map_physical(id, pipeline);
				}
				_ => {}
			}
		}

		pipeline_map
	}

	fn alloc_compute_pipelines(graph: &mut RenderGraph, graphics_device: &mut GraphicsDevice) -> VirtualToPhysicalResourceMap<usize> {
		let mut pipeline_map = VirtualToPhysicalResourceMap::new();

		for (id, resource) in graph.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::ComputePipeline { cs, descriptor_layouts, .. } => {
					// TODO(Brandon): Definitely don't do it like this, this is a hack to get the raw pointer
					let cs = match &graph.imported_resources[cs.id] {
						GraphImportedResource::Shader(shader) => shader.module,
						_ => panic!("Invalid compute shader handle!"),
					};

					let descriptor_layouts = descriptor_layouts
						.into_iter()
						.map(|info| graph.cache.register_compute_descriptor_layout(graphics_device, info))
						.collect::<Vec<_>>();

					let key = ComputePipelineCacheKey { cs, descriptor_layouts };

					let pipeline = graph.cache.alloc_compute_pipeline(graphics_device, &key);
					pipeline_map.map_physical(id, pipeline);
				}
				_ => {}
			}
		}

		pipeline_map
	}
}

impl<'a> RenderGraph<'a> {
	pub fn new(cache: &'a mut RenderGraphCache) -> Self {
		Self {
			passes: Default::default(),
			owned_resources: Default::default(),
			resource_to_owning_pass: Default::default(),
			imported_resources: Default::default(),
			cache,
		}
	}

	pub fn add_pass<'b>(&'b mut self, name: &'static str) -> PassBuilder<'a, 'b> {
		let pass = PassHandle { id: self.passes.len() };
		let recorded = Some(RecordedPass {
			name,
			pass,
			cmds: Default::default(),
			read_attachments: Default::default(),
			write_attachments: Default::default(),
			read_buffers: Default::default(),
			write_buffers: Default::default(),
		});

		PassBuilder { graph: self, pass, recorded }
	}

	fn resolve_pass_dependencies(&mut self, pass: PassHandle, pass_order: &mut Vec<PassHandle>) -> PassDependencyNode {
		let recorded_pass = &self.passes[pass.id];
		pass_order.push(pass);
		let dependencies = recorded_pass
			.read_attachments
			.iter()
			.map(|a| self.resource_to_owning_pass[&a.id])
			.chain(recorded_pass.read_buffers.iter().map(|b| self.resource_to_owning_pass[&b.id]))
			// .chain(recorded_pass.write_attachments.iter().map(|a| self.resource_to_owning_pass[&a.id]))
			.collect::<HashSet<_>>()
			.into_iter()
			.map(|p| self.resolve_pass_dependencies(p, pass_order))
			.collect::<Vec<_>>();

		PassDependencyNode { pass, dependencies }
	}

	pub fn execute(mut self, graphics_context: &mut GraphicsContext, graphics_device: &mut GraphicsDevice) {
		let output = self
			.owned_resources
			.iter()
			.enumerate()
			.filter(|(_, r)| matches!(r, GraphOwnedResource::OutputRenderPass {}))
			.collect::<Vec<_>>();

		if output.len() != 1 {
			panic!("Multiple output render pass resources were found!");
		}

		let (id, _) = output[0];

		let root_pass = self.resource_to_owning_pass[&id];
		let mut passes = vec![root_pass];
		// let root =
		self.resolve_pass_dependencies(root_pass, &mut passes);
		// dbg!(root);

		let mut found = HashSet::<PassHandle>::new();
		passes.reverse();
		passes.retain(|p| found.insert(*p));

		let resource_map = GraphPhysicalResourceMap::new(&mut self, graphics_device, graphics_context);
		for pass in passes {
			for cmd in self.passes[pass.id].cmds.iter() {
				match cmd {
					PassCmd::BeginRenderPass { render_pass, clear_values } => {
						for &attachment in self.passes[pass.id].read_attachments.iter() {
							let physical_attachment = resource_map.get_attachment(&self, attachment);

							graphics_context.pipeline_barrier(
								attachment.src_stage,
								attachment.dst_stage,
								ash::vk::DependencyFlags::empty(),
								&[],
								&[],
								&[ash::vk::ImageMemoryBarrier::builder()
									.old_layout(attachment.initial_layout.into())
									.new_layout(attachment.final_layout.into())
									.image(physical_attachment.image)
									.subresource_range(physical_attachment.subresource_range)
									.src_access_mask(attachment.src_access)
									.dst_access_mask(attachment.dst_access)
									.src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
									.dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
									.build()],
							);
						}

						for &buffer in self.passes[pass.id].read_buffers.iter() {
							let physical_buffer = resource_map.get_buffer(&self, buffer);

							graphics_context.pipeline_barrier(
								buffer.src_stage,
								buffer.dst_stage,
								ash::vk::DependencyFlags::empty(),
								&[],
								&[ash::vk::BufferMemoryBarrier::builder()
									.buffer(physical_buffer.raw)
									.size(physical_buffer.size as u64)
									.offset(0)
									.src_access_mask(buffer.src_access)
									.dst_access_mask(buffer.dst_access)
									.src_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
									.dst_queue_family_index(ash::vk::QUEUE_FAMILY_IGNORED)
									.build()],
								&[],
							)
						}

						if let Some((render_pass, framebuffer)) = resource_map.get_render_pass(&self, *render_pass) {
							graphics_context.begin_render_pass(render_pass, framebuffer, &clear_values);
						} else {
							graphics_context.begin_output_render_pass(&clear_values);
						}
					}
					PassCmd::EndRenderPass {} => graphics_context.end_render_pass(),
					&PassCmd::BindRasterPipeline { pipeline } => {
						let pipeline = resource_map.get_raster_pipeline(&self, pipeline);
						graphics_context.bind_raster_pipeline(pipeline);
					}
					&PassCmd::BindComputePipeline { pipeline } => {
						let pipeline = resource_map.get_compute_pipeline(&self, pipeline);
						graphics_context.bind_compute_pipeline(pipeline);
					}
					&PassCmd::BindDescriptor { set, descriptor, pipeline } => {
						let pipeline = match pipeline {
							GraphPipelineHandle::Raster(pipeline) => resource_map.get_raster_pipeline(&self, pipeline),
							GraphPipelineHandle::Compute(pipeline) => resource_map.get_compute_pipeline(&self, pipeline),
						};

						let (descriptor, descriptor_heap) = resource_map.get_descriptor(&self, descriptor);

						graphics_context.bind_descriptor(descriptor_heap, &descriptor, set, pipeline);
					}
					PassCmd::DrawMesh { mesh } => match &self.imported_resources[mesh.id] {
						GraphImportedResource::Mesh(mesh) => graphics_context.draw_mesh(mesh),
						_ => unreachable!("Invalid mesh!"),
					},
					&PassCmd::Draw {
						vertex_count,
						instance_count,
						first_vertex,
						first_instance,
					} => graphics_context.draw(vertex_count, instance_count, first_vertex, first_instance),
				}
			}
		}
	}

	fn import_resource(&mut self, resource: GraphImportedResource<'a>) -> usize {
		if let Some(index) = self.imported_resources.iter().position(|&r| resource == r) {
			return index;
		}

		self.imported_resources.push(resource);

		self.imported_resources.len() - 1
	}

	fn create_resource(&mut self, pass: PassHandle, resource: GraphOwnedResource) -> usize {
		let id = self.owned_resources.len();
		self.owned_resources.push(resource);
		self.resource_to_owning_pass.insert(id, pass);

		id
	}

	fn record_pass(&mut self, pass: RecordedPass) {
		self.passes.push(pass);
	}
}

pub struct PassBuilder<'a, 'b> {
	graph: &'b mut RenderGraph<'a>,
	pass: PassHandle,
	recorded: Option<RecordedPass>,
}

impl<'a, 'b> PassBuilder<'a, 'b> {
	pub fn add_attachment(&mut self, desc: AttachmentDesc) -> MutableGraphAttachmentHandle {
		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::Attachment {
				name: desc.name,
				width: desc.width,
				height: desc.height,
				format: desc.format,
				load_op: desc.load_op,
				store_op: desc.store_op,
				usage: desc.usage,
			},
		);

		MutableGraphAttachmentHandle {
			id,
			layout: ImageLayout::Undefined,
			stage: ash::vk::PipelineStageFlags::empty(),
			access: ash::vk::AccessFlags::empty(),
		}
	}

	pub fn add_buffer(&mut self, desc: BufferDesc) -> MutableGraphBufferHandle {
		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::Buffer {
				name: desc.name,
				size: desc.size,
				usage: desc.usage,
				location: desc.location,
			},
		);

		MutableGraphBufferHandle {
			id,
			stage: ash::vk::PipelineStageFlags::empty(),
			access: ash::vk::AccessFlags::empty(),
		}
	}

	fn decl_read_attachment(&mut self, attachment: GraphAttachmentHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.read_attachments.insert(attachment);
	}

	fn decl_write_attachment(&mut self, attachment: MutableGraphAttachmentHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.write_attachments.insert(attachment);
	}

	fn decl_read_buffer(&mut self, buffer: GraphBufferHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.read_buffers.insert(buffer);
	}

	fn decl_write_buffer(&mut self, buffer: MutableGraphBufferHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.write_buffers.insert(buffer);
	}

	pub fn add_raster_pipeline<'c>(&mut self, desc: RasterPipelineDesc<'a, 'c>) -> GraphRasterPipelineHandle {
		let name = desc.name;

		let vs = GraphImportedShaderHandle {
			id: self.graph.import_resource(GraphImportedResource::Shader(desc.vs)),
		};

		let ps = if let Some(ps) = desc.ps {
			Some(GraphImportedShaderHandle {
				id: self.graph.import_resource(GraphImportedResource::Shader(ps)),
			})
		} else {
			None
		};

		let descriptor_layouts = desc.descriptor_layouts.to_vec();
		let render_pass = desc.render_pass;
		let depth_compare_op = desc.depth_compare_op;
		let depth_write = desc.depth_write;
		let face_cull = desc.face_cull;
		let push_constant_bytes = desc.push_constant_bytes;
		let vertex_input_info = desc.vertex_input_info;
		let polygon_mode = desc.polygon_mode;

		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::RasterPipeline {
				name,
				vs,
				ps,
				descriptor_layouts,
				render_pass,
				depth_compare_op,
				depth_write,
				face_cull,
				push_constant_bytes,
				vertex_input_info,
				polygon_mode,
			},
		);

		GraphRasterPipelineHandle { id }
	}

	pub fn add_render_pass(&mut self, desc: RenderPassDesc) -> GraphRenderPassHandle {
		let recorded = self.recorded.as_mut().unwrap();

		let name = desc.name;
		let color_attachments = desc
			.color_attachments
			.into_iter()
			.map(|a| {
				a.layout = ImageLayout::ColorAttachmentOptimal;
				a.access = ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
				a.stage = ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
				recorded.write_attachments.insert(**a);
				**a
			})
			.collect::<Vec<_>>();

		let depth_attachment = desc.depth_attachment.map_or(None, |a| {
			a.layout = ImageLayout::DepthStencilAttachmentOptimal;
			a.access = ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
			a.stage = ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
			recorded.write_attachments.insert(*a);
			Some(*a)
		});

		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::RenderPass {
				name,
				color_attachments,
				depth_attachment,
			},
		);

		GraphRenderPassHandle { id }
	}

	pub fn add_output_render_pass(&mut self) -> GraphRenderPassHandle {
		let id = self.graph.create_resource(self.pass, GraphOwnedResource::OutputRenderPass {});

		GraphRenderPassHandle { id }
	}

	pub fn add_descriptor_set<'c>(&mut self, desc: DescriptorDesc<'a, 'c>) -> GraphDescriptorHandle {
		let name = desc.name;
		let bindings = desc
			.bindings
			.into_iter()
			.map(|(i, binding)| {
				(
					*i,
					match binding {
						DescriptorBindingDesc::ImportedBuffer(buffer) => {
							let id = self.graph.import_resource(GraphImportedResource::Buffer(buffer));
							GraphOwnedResourceDescriptorBinding::ImportedBuffer(GraphImportedBufferHandle { id })
						}
						DescriptorBindingDesc::ImportedTexture(texture) => {
							let id = self.graph.import_resource(GraphImportedResource::Texture(texture));
							GraphOwnedResourceDescriptorBinding::ImportedTexture(GraphImportedTextureHandle { id })
						}
						DescriptorBindingDesc::Buffer(buffer) => {
							self.decl_read_buffer(*buffer);
							GraphOwnedResourceDescriptorBinding::Buffer(*buffer)
						}
						DescriptorBindingDesc::MutableBuffer(buffer) => {
							buffer.stage = ash::vk::PipelineStageFlags::VERTEX_SHADER | ash::vk::PipelineStageFlags::FRAGMENT_SHADER | ash::vk::PipelineStageFlags::COMPUTE_SHADER;
							buffer.access = ash::vk::AccessFlags::SHADER_READ | ash::vk::AccessFlags::SHADER_WRITE;
							self.decl_write_buffer(**buffer);
							GraphOwnedResourceDescriptorBinding::MutableBuffer(**buffer)
						}
						DescriptorBindingDesc::Attachment(attachment) => {
							self.decl_read_attachment(*attachment);
							GraphOwnedResourceDescriptorBinding::Attachment(*attachment)
						}
						DescriptorBindingDesc::MutableAttachment(attachment) => {
							unimplemented!();
							attachment.layout = ImageLayout::General;
							attachment.stage = ash::vk::PipelineStageFlags::VERTEX_SHADER | ash::vk::PipelineStageFlags::FRAGMENT_SHADER | ash::vk::PipelineStageFlags::COMPUTE_SHADER;
							attachment.access = ash::vk::AccessFlags::SHADER_READ | ash::vk::AccessFlags::SHADER_WRITE;
							self.decl_write_attachment(**attachment);
							GraphOwnedResourceDescriptorBinding::MutableAttachment(**attachment)
						}
					},
				)
			})
			.collect::<Vec<_>>();

		// TODO(Brandon): Validate bindings with descriptor set info.

		let descriptor_layout = desc.descriptor_layout;

		let id = self.graph.create_resource(self.pass, GraphOwnedResource::DescriptorSet { name, bindings, descriptor_layout });

		GraphDescriptorHandle { id }
	}

	pub fn cmd_begin_render_pass(&mut self, render_pass: GraphRenderPassHandle, clear_values: &[ClearValue]) {
		let recorded = self.recorded.as_mut().unwrap();
		let clear_values = clear_values.to_vec();

		recorded.cmds.push(PassCmd::BeginRenderPass { render_pass, clear_values });
	}

	pub fn cmd_bind_raster_pipeline(&mut self, pipeline: GraphRasterPipelineHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BindRasterPipeline { pipeline });
	}

	pub fn cmd_bind_raster_descriptor(&mut self, descriptor: GraphDescriptorHandle, set: u32, pipeline: GraphRasterPipelineHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BindDescriptor {
			set,
			descriptor,
			pipeline: GraphPipelineHandle::Raster(pipeline),
		});
	}

	pub fn cmd_bind_compute_descriptor(&mut self, descriptor: GraphDescriptorHandle, set: u32, pipeline: GraphComputePipelineHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BindDescriptor {
			set,
			descriptor,
			pipeline: GraphPipelineHandle::Compute(pipeline),
		});
	}

	pub fn cmd_draw_mesh(&mut self, mesh: &'a Mesh) {
		let id = self.graph.import_resource(GraphImportedResource::Mesh(mesh));
		let mesh = GraphImportedMeshHandle { id };

		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::DrawMesh { mesh });
	}

	pub fn cmd_draw(&mut self, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::Draw {
			vertex_count,
			instance_count,
			first_vertex,
			first_instance,
		});
	}

	pub fn cmd_end_render_pass(&mut self) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::EndRenderPass {});
	}
}

impl<'a, 'b> Drop for PassBuilder<'a, 'b> {
	fn drop(&mut self) {
		self.graph.record_pass(self.recorded.take().unwrap());
	}
}
