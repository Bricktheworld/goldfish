use super::*;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub enum PassCmd {
	BeginRenderPass {
		render_pass: GraphRenderPassHandle,
		clear_value: Color,
	},
	EndRenderPass {},
	BindRasterPipeline {
		pipeline: GraphRasterPipelineHandle,
	},
	BindDescriptor {
		set: u32,
		descriptor: GraphDescriptorHandle,
		pipeline: GraphRasterPipelineHandle,
	},
	DrawMesh {
		mesh: GraphImportedMeshHandle,
	}, // BindPipeline(vk::PipelineBindPoint, vk::Pipeline),
	   // BindVertexBuffer(u32, vk::Buffer, vk::DeviceSize),
	   // BindVertexBuffers(u32, Vec<vk::Buffer>, Vec<vk::DeviceSize>),
	   // BindIndexBuffer(vk::Buffer, vk::DeviceSize, vk::IndexType),
	   // SetViewport(vk::Viewport),
	   // SetScissor(vk::Rect2D),
	   // DrawIndexed(u32, u32, u32, i32, u32),
	   // BindDescriptor(
	   // 	vk::PipelineBindPoint,
	   // 	vk::PipelineLayout,
	   // 	u32,
	   // 	vk::DescriptorSet,
	   // ),
	   // None,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct BufferCacheKey {}

#[derive(Default)]
struct BufferCache {}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct TextureCacheKey {
	width: u32,
	height: u32,
	format: TextureFormat,
	usage: TextureUsage,
}

#[derive(Default)]
struct TextureCache {
	cache: HashMap<TextureCacheKey, Vec<Texture>>,
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
	cache: HashMap<FramebufferCacheKey, Framebuffer>,
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
	vs: ash::vk::ShaderModule, // TODO(Brandon): Make this platform agnostic or find some better way to do this.
	ps: ash::vk::ShaderModule, // This applies to all borrowed resources where we need some hashable way of identifying them.
	descriptor_layouts: Vec<DescriptorLayout>,
	render_pass: usize,
	depth_write: bool,
	face_cull: bool,
	push_constant_bytes: usize,
}

#[derive(Default)]
struct RasterPipelineCache {
	pipelines: Vec<Pipeline>,
	cache: HashMap<RasterPipelineCacheKey, usize>,
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
	OwnedBuffer {
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
	texture_cache: TextureCache,
	attachment_cache: AttachmentCache,
	framebuffer_cache: FramebufferCache,
	render_pass_cache: RenderPassCache,
	raster_pipeline_cache: RasterPipelineCache,
	descriptor_layout_cache: DescriptorLayoutCache,
	descriptor_heap_caches: HashMap<*const DescriptorSetInfo, DescriptorHeapCache>,
}

impl RenderGraphCache {
	fn alloc_render_pass(
		&mut self,
		graphics_device: &GraphicsDevice,
		key: &RenderPassCacheKey,
	) -> usize {
		*self
			.render_pass_cache
			.cache
			.entry(key.clone())
			.or_insert_with(|| {
				println!("Allocated render pass!");
				self.render_pass_cache.render_passes.push(
					graphics_device
						.create_render_pass(&key.color_attachment_descs, key.depth_attachment_desc),
				);

				self.render_pass_cache.render_passes.len() - 1
			})
	}

	fn get_render_pass_index(&self, key: &RenderPassCacheKey) -> usize {
		*self.render_pass_cache.cache.get(key).unwrap()
	}

	fn get_render_pass(&self, key: &RenderPassCacheKey) -> &RenderPass {
		&self.render_pass_cache.render_passes[self.get_render_pass_index(key)]
	}

	fn get_framebuffer(&self, key: &FramebufferCacheKey) -> &Framebuffer {
		self.framebuffer_cache.cache.get(key).unwrap()
	}

	fn alloc_framebuffer(&mut self, graphics_device: &GraphicsDevice, key: &FramebufferCacheKey) {
		self.framebuffer_cache
			.cache
			.entry(key.clone())
			.or_insert_with(|| {
				println!("Allocated framebuffer!");
				let render_pass = &self.render_pass_cache.render_passes[key.render_pass];
				let attachments = key
					.attachments
					.iter()
					.map(|a| &self.attachment_cache.attachments[*a])
					.collect::<Vec<_>>();
				graphics_device.create_framebuffer(key.width, key.height, render_pass, &attachments)
			});
	}

	fn alloc_raster_pipeline(
		&mut self,
		graphics_context: &mut GraphicsContext,
		graphics_device: &GraphicsDevice,
		key: &RasterPipelineCacheKey,
	) {
		self.raster_pipeline_cache
			.cache
			.entry(key.clone())
			.or_insert_with(|| {
				println!("Allocated pipeline!");

				// TODO(Brandon): Really good example of how we should allow for fetching of the render pass from the swapchain.
				self.raster_pipeline_cache
					.pipelines
					.push(if key.render_pass == usize::MAX {
						graphics_context.create_raster_pipeline(
							&Shader { module: key.vs },
							&Shader { module: key.ps },
							&key.descriptor_layouts,
							key.depth_write,
							key.face_cull,
							key.push_constant_bytes,
						)
					} else {
						graphics_device.create_raster_pipeline(
							&Shader { module: key.vs },
							&Shader { module: key.ps },
							&key.descriptor_layouts,
							&mut self.render_pass_cache.render_passes[key.render_pass],
							key.depth_write,
							key.face_cull,
							key.push_constant_bytes,
						)
					});

				self.raster_pipeline_cache.pipelines.len() - 1
			});
	}

	fn get_raster_pipeline_index(&self, key: &RasterPipelineCacheKey) -> usize {
		*self.raster_pipeline_cache.cache.get(key).unwrap()
	}

	fn get_raster_pipeline(&self, key: &RasterPipelineCacheKey) -> &Pipeline {
		&self.raster_pipeline_cache.pipelines[self.get_raster_pipeline_index(key)]
	}

	fn alloc_attachments(
		&mut self,
		graphics_device: &GraphicsDevice,
		key: &AttachmentCacheKey,
		count: usize,
	) {
		let attachments = self.attachment_cache.cache.entry(key.clone()).or_default();
		while attachments.len() < count {
			println!("Allocated attachment!");
			attachments.push(self.attachment_cache.attachments.len());
			self.attachment_cache
				.attachments
				.push(graphics_device.create_texture(
					key.width,
					key.height,
					key.format,
					key.usage | TextureUsage::ATTACHMENT,
				));
		}
	}

	fn alloc_descriptor(
		&mut self,
		graphics_device: &GraphicsDevice,
		descriptor_info: &'static DescriptorSetInfo,
		key: &DescriptorHeapCacheKey,
	) -> DescriptorHandle {
		self.register_graphics_descriptor_layout(graphics_device, descriptor_info);

		let descriptor_cache = self
			.descriptor_heap_caches
			.get_mut(&(descriptor_info as *const DescriptorSetInfo))
			.unwrap();

		*descriptor_cache
			.cache
			.entry(key.clone())
			.or_insert_with(|| {
				println!("Allocated descriptor!");
				descriptor_cache.heap.alloc().unwrap()
			})
	}

	fn register_graphics_descriptor_layout(
		&mut self,
		graphics_device: &GraphicsDevice,
		descriptor_info: &'static DescriptorSetInfo,
	) -> DescriptorLayout {
		let layout =
			graphics_device.get_graphics_layout(&mut self.descriptor_layout_cache, descriptor_info);

		self.descriptor_heap_caches
			.entry(descriptor_info)
			.or_insert_with(|| DescriptorHeapCache {
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

		for (_, framebuffer) in self.framebuffer_cache.cache {
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

#[derive(Clone)]
pub struct RenderPassDesc {
	pub name: &'static str,
	pub color_attachments: Vec<MutableGraphAttachmentHandle>,
	pub depth_attachment: Option<MutableGraphAttachmentHandle>,
}

#[derive(Clone)]
pub struct RasterPipelineDesc<'a> {
	pub name: &'static str,
	pub vs: &'a Shader,
	pub ps: &'a Shader,
	pub descriptor_layouts: Vec<&'static DescriptorSetInfo>,
	pub render_pass: GraphRenderPassHandle,
	pub depth_write: bool,
	pub face_cull: bool,
	pub push_constant_bytes: usize,
}

#[derive(Clone, Copy)]
pub enum DescriptorBindingDesc<'a> {
	ImportedBuffer(&'a GpuBuffer),
	ImportedTexture(&'a Texture),
	OwnedBuffer(GraphBufferHandle),
	Attachment(GraphAttachmentHandle),
	MutableAttachment(MutableGraphAttachmentHandle),
}

#[derive(Clone)]
pub struct DescriptorDesc<'a> {
	pub name: &'static str,
	pub descriptor_layout: &'static DescriptorSetInfo,
	pub bindings: Vec<(u32, DescriptorBindingDesc<'a>)>,
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
	OwnedBuffer(GraphBufferHandle),
	Attachment(GraphAttachmentHandle),
	MutableAttachment(MutableGraphAttachmentHandle),
}

#[derive(Debug, Clone)]
enum GraphOwnedResource {
	RasterPipeline {
		name: &'static str,
		vs: GraphImportedShaderHandle,
		ps: GraphImportedShaderHandle,
		descriptor_layouts: Vec<&'static DescriptorSetInfo>,
		render_pass: GraphRenderPassHandle,
		depth_write: bool,
		face_cull: bool,
		push_constant_bytes: usize,
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
pub struct GraphAttachmentHandle {
	id: usize,
	layout: ImageLayout,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphBufferHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MutableGraphAttachmentHandle {
	id: usize,
	layout: ImageLayout,
}

impl GraphAttachmentHandle {
	fn upgrade(self) -> MutableGraphAttachmentHandle {
		MutableGraphAttachmentHandle {
			id: self.id,
			layout: self.layout,
		}
	}
}

impl From<MutableGraphAttachmentHandle> for GraphAttachmentHandle {
	fn from(mutable: MutableGraphAttachmentHandle) -> Self {
		Self {
			id: mutable.id,
			layout: mutable.layout,
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
	pub name: &'static str,
	pass: PassHandle,
	pub cmds: Vec<PassCmd>,
	pub read_attachments: Vec<GraphAttachmentHandle>,
	pub write_attachments: Vec<MutableGraphAttachmentHandle>,
	pub render_attachments: Vec<MutableGraphAttachmentHandle>,
}

pub struct RenderGraph<'a> {
	passes: Vec<RecordedPass>,
	owned_resources: Vec<GraphOwnedResource>,
	resource_to_owning_pass: HashMap<usize, PassHandle>,
	imported_resources: Vec<GraphImportedResource<'a>>,
	cache: &'a mut RenderGraphCache,
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
		let pass = PassHandle {
			id: self.passes.len(),
		};
		let recorded = Some(RecordedPass {
			name,
			pass,
			cmds: Default::default(),
			read_attachments: Default::default(),
			write_attachments: Default::default(),
			render_attachments: Default::default(),
		});

		PassBuilder {
			graph: self,
			pass,
			recorded,
		}
	}

	fn resolve_pass_dependencies(
		&self,
		pass: PassHandle,
		pass_order: &mut Vec<PassHandle>,
	) -> PassDependencyNode {
		let recorded_pass = &self.passes[pass.id];
		pass_order.push(pass);
		let dependencies = recorded_pass
			.read_attachments
			.iter()
			.map(|a| self.resource_to_owning_pass[&a.id])
			.chain(
				recorded_pass
					.write_attachments
					.iter()
					.map(|a| self.resource_to_owning_pass[&a.id]),
			)
			.collect::<HashSet<_>>()
			.into_iter()
			.map(|p| self.resolve_pass_dependencies(p, pass_order))
			.collect::<Vec<_>>();

		PassDependencyNode { pass, dependencies }
	}

	fn alloc_attachments(&mut self, graphics_device: &mut GraphicsDevice) -> HashMap<usize, usize> {
		let mut attachment_type_to_virtual = HashMap::<AttachmentCacheKey, Vec<usize>>::new();

		for (i, resource) in self.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::Attachment {
					width,
					height,
					format,
					usage,
					..
				} => {
					let key = AttachmentCacheKey {
						width: *width,
						height: *height,
						format: *format,
						usage: *usage,
					};

					attachment_type_to_virtual.entry(key).or_default().push(i);
				}
				_ => {}
			}
		}

		for (key, virtual_resources) in attachment_type_to_virtual.iter() {
			self.cache
				.alloc_attachments(graphics_device, key, virtual_resources.len());
		}

		let mut virtual_to_physical_attachments = HashMap::<_, _>::new();

		// TODO(Brandon): Optimize this by mapping virtual to physical attachments based on existing framebuffers to reduce framebuffer allocation.
		for (key, virtual_resources) in attachment_type_to_virtual {
			for (i, virtual_resource) in virtual_resources.into_iter().enumerate() {
				let index = self.cache.attachment_cache.cache[&key][i];
				virtual_to_physical_attachments.insert(virtual_resource, index);
			}
		}

		virtual_to_physical_attachments
	}

	fn alloc_descriptors(
		&mut self,
		graphics_device: &mut GraphicsDevice,
		virtual_to_physical_attachents: &HashMap<usize, usize>,
	) -> HashMap<usize, DescriptorHandle> {
		let mut virtual_to_physical_descriptors = HashMap::<_, _>::new();
		for (i, resource) in self.owned_resources.iter().enumerate() {
			match resource {
				GraphOwnedResource::DescriptorSet {
					descriptor_layout,
					bindings,
					..
				} => {
					let bindings = bindings
						.iter()
						.map(|(i, binding)| {
							(
								*i,
								match binding {
									GraphOwnedResourceDescriptorBinding::ImportedBuffer(buffer) => {
										match &self.imported_resources[buffer.id] {
											GraphImportedResource::Buffer(buffer) => {
												DescriptorHeapCacheKeyBinding::ImportedBuffer {
													buffer: buffer.raw,
												}
											}
											_ => unreachable!("Invalid buffer handle!"),
										}
									}
									GraphOwnedResourceDescriptorBinding::ImportedTexture(
										texture,
									) => match &self.imported_resources[texture.id] {
										GraphImportedResource::Texture(texture) => {
											DescriptorHeapCacheKeyBinding::ImportedTexture {
												image: texture.image,
												sampler: texture.sampler,
												image_view: texture.image_view,
											}
										}
										_ => unreachable!("Invalid texture handle!"),
									},
									GraphOwnedResourceDescriptorBinding::OwnedBuffer(buffer) => {
										unimplemented!()
										// DescriptorHeapCacheKeyBinding::OwnedBuffer { buffer: buffer.id }
									}
									GraphOwnedResourceDescriptorBinding::Attachment(attachment) => {
										DescriptorHeapCacheKeyBinding::Attachment {
											attachment: virtual_to_physical_attachents
												[&attachment.id],
										}
									}
									GraphOwnedResourceDescriptorBinding::MutableAttachment(
										attachment,
									) => DescriptorHeapCacheKeyBinding::Attachment {
										attachment: virtual_to_physical_attachents[&attachment.id],
									},
								},
							)
						})
						.collect::<Vec<_>>();
					let key = DescriptorHeapCacheKey { bindings };
					let descriptor =
						self.cache
							.alloc_descriptor(graphics_device, descriptor_layout, &key);
					virtual_to_physical_descriptors
						.entry(i)
						.or_insert(descriptor);
				}
				_ => {}
			}
		}

		virtual_to_physical_descriptors
	}

	pub fn execute(
		mut self,
		graphics_context: &mut GraphicsContext,
		graphics_device: &mut GraphicsDevice,
	) {
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

		let virtual_to_physical_attachments = self.alloc_attachments(graphics_device);
		let virtual_to_physical_descriptors =
			self.alloc_descriptors(graphics_device, &virtual_to_physical_attachments);

		let mut attachment_layouts = virtual_to_physical_attachments
			.iter()
			.map(|(resource, _)| (*resource, ImageLayout::Undefined))
			.collect::<HashMap<_, _>>();

		let mut get_attachment_description = |attachment_handle: MutableGraphAttachmentHandle| {
			let layout = attachment_layouts.get_mut(&attachment_handle.id).unwrap();
			let initial_layout = *layout;
			let final_layout = attachment_handle.layout;
			*layout = final_layout;

			match &self.owned_resources[attachment_handle.id] {
				GraphOwnedResource::Attachment {
					format,
					usage,
					load_op,
					store_op,
					..
				} => AttachmentDescription {
					format: *format,
					usage: *usage,
					load_op: *load_op,
					store_op: *store_op,
					initial_layout,
					final_layout,
				},
				_ => unreachable!("Invalid attachment for render pass!"),
			}
		};

		let mut virtual_to_physical_render_passes = HashMap::<usize, usize>::new();
		let mut virtual_to_physical_raster_pipelines = HashMap::<usize, usize>::new();
		let mut updated_descriptors = HashSet::<usize>::new();

		for pass in passes {
			let pass = &self.passes[pass.id];
			for cmd in pass.cmds.iter() {
				match cmd {
					PassCmd::BeginRenderPass {
						render_pass,
						clear_value,
					} => match &self.owned_resources[render_pass.id] {
						GraphOwnedResource::RenderPass {
							color_attachments,
							depth_attachment,
							..
						} => {
							let render_pass_key = RenderPassCacheKey {
								color_attachment_descs: color_attachments
									.iter()
									.map(|attachment_handle| {
										get_attachment_description(*attachment_handle)
									})
									.collect::<Vec<_>>(),
								depth_attachment_desc: depth_attachment.map_or(
									None,
									|attachment_handle| {
										Some(get_attachment_description(attachment_handle))
									},
								),
							};

							let render_pass_index = self
								.cache
								.alloc_render_pass(graphics_device, &render_pass_key);

							virtual_to_physical_render_passes
								.entry(render_pass.id)
								.or_insert(render_pass_index);

							let width = color_attachments
								.iter()
								.map(|a| match &self.owned_resources[a.id] {
									GraphOwnedResource::Attachment { width, .. } => *width,
									_ => unreachable!(),
								})
								.min()
								.unwrap_or(0);

							let height = color_attachments
								.iter()
								.map(|a| match &self.owned_resources[a.id] {
									GraphOwnedResource::Attachment { height, .. } => *height,
									_ => unreachable!(),
								})
								.min()
								.unwrap_or(0);

							let mut framebuffer_attachment_indices = color_attachments
								.iter()
								.map(|a| virtual_to_physical_attachments[&a.id])
								.collect::<Vec<_>>();

							if let Some(depth_attachment) = depth_attachment {
								framebuffer_attachment_indices
									.push(virtual_to_physical_attachments[&depth_attachment.id]);
							}

							let framebuffer_key = FramebufferCacheKey {
								width,
								height,
								attachments: framebuffer_attachment_indices,
								render_pass: render_pass_index,
							};
							self.cache
								.alloc_framebuffer(graphics_device, &framebuffer_key);

							let render_pass = self.cache.get_render_pass(&render_pass_key);
							let framebuffer = self.cache.get_framebuffer(&framebuffer_key);

							graphics_context.begin_render_pass(
								render_pass,
								framebuffer,
								*clear_value,
							);
						}
						GraphOwnedResource::OutputRenderPass {} => {
							graphics_context.begin_output_render_pass(*clear_value);
							virtual_to_physical_render_passes.insert(render_pass.id, usize::MAX);
						}
						_ => panic!("Invalid render pass handle!"),
					},
					PassCmd::EndRenderPass {} => {
						graphics_context.end_render_pass();
					}
					PassCmd::BindRasterPipeline { pipeline } => {
						match &self.owned_resources[pipeline.id] {
							GraphOwnedResource::RasterPipeline {
								vs,
								ps,
								descriptor_layouts,
								render_pass,
								depth_write,
								face_cull,
								push_constant_bytes,
								..
							} => {
								// TODO(Brandon): Definitely don't do it like this, this is a hack to get the raw pointer
								let vs = match &self.imported_resources[vs.id] {
									GraphImportedResource::Shader(shader) => shader.module,
									_ => panic!("Invalid vertex shader handle!"),
								};

								let ps = match &self.imported_resources[ps.id] {
									GraphImportedResource::Shader(shader) => shader.module,
									_ => panic!("Invalid vertex shader handle!"),
								};

								let descriptor_layouts = descriptor_layouts
									.into_iter()
									.map(|info| {
										self.cache.register_graphics_descriptor_layout(
											graphics_device,
											info,
										)
									})
									.collect::<Vec<_>>();

								let render_pass = *virtual_to_physical_render_passes
									.get(&render_pass.id)
									.expect("Raster pipeline not found. Perhaps it was not bound or the wrong one was bound?");

								let key = RasterPipelineCacheKey {
									vs,
									ps,
									depth_write: *depth_write,
									face_cull: *face_cull,
									push_constant_bytes: *push_constant_bytes,
									render_pass,
									descriptor_layouts,
								};

								self.cache.alloc_raster_pipeline(
									graphics_context,
									graphics_device,
									&key,
								);

								let physical_index = self.cache.get_raster_pipeline_index(&key);

								virtual_to_physical_raster_pipelines
									.entry(pipeline.id)
									.or_insert(physical_index);

								let pipeline =
									&self.cache.raster_pipeline_cache.pipelines[physical_index];
								graphics_context.bind_raster_pipeline(pipeline);
							}
							_ => panic!("Invalid raster pipeline!"),
						}
					}
					PassCmd::BindDescriptor {
						descriptor,
						set,
						pipeline,
					} => match &self.owned_resources[descriptor.id] {
						GraphOwnedResource::DescriptorSet {
							descriptor_layout,
							bindings,
							..
						} => {
							let descriptor_heap = &self
								.cache
								.descriptor_heap_caches
								.get(&(*descriptor_layout as *const DescriptorSetInfo))
								.unwrap()
								.heap;

							let physical_descriptor =
								&virtual_to_physical_descriptors[&descriptor.id];

							if updated_descriptors.insert(descriptor.id) {
								let buffers = bindings
									.iter()
									.filter(|(_, ty)| match ty {
										GraphOwnedResourceDescriptorBinding::ImportedBuffer(..) => {
											true
										}
										_ => false,
									})
									.map(|(binding, buffer)| {
										(*binding, match buffer {
										GraphOwnedResourceDescriptorBinding::ImportedBuffer(
											buffer,
										) => match self.imported_resources[buffer.id] {
											GraphImportedResource::Buffer(buffer) => buffer,
											_ => unreachable!("Invalid imported buffer!"),
										},
										_ => unreachable!(),
									})
									})
									.collect::<Vec<_>>();

								graphics_context.update_descriptor_buffers(
									&buffers,
									descriptor_layout,
									descriptor_heap,
									physical_descriptor,
								);
							}

							let physical_pipeline_index =
								virtual_to_physical_raster_pipelines[&pipeline.id];
							let physical_pipeline = &self.cache.raster_pipeline_cache.pipelines
								[physical_pipeline_index];

							graphics_context.bind_descriptor(
								descriptor_heap,
								physical_descriptor,
								*set,
								physical_pipeline,
							);
						}
						_ => unreachable!("Invalid descriptor!"),
					},
					PassCmd::DrawMesh { mesh } => match &self.imported_resources[mesh.id] {
						GraphImportedResource::Mesh(mesh) => graphics_context.draw_mesh(mesh),
						_ => unreachable!("Invalid mesh!"),
					},
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
		}
	}

	pub fn decl_read_attachment(
		&mut self,
		mut attachment: GraphAttachmentHandle,
		layout: ImageLayout,
	) {
		let recorded = self.recorded.as_mut().unwrap();
		attachment.layout = layout;
		recorded.read_attachments.push(attachment);
	}

	pub fn decl_write_attachment(
		&mut self,
		mut attachment: MutableGraphAttachmentHandle,
		layout: ImageLayout,
	) {
		let recorded = self.recorded.as_mut().unwrap();
		attachment.layout = layout;
		recorded.write_attachments.push(attachment);
	}

	pub fn add_raster_pipeline(
		&mut self,
		desc: RasterPipelineDesc<'a>,
	) -> GraphRasterPipelineHandle {
		let name = desc.name;

		let vs = GraphImportedShaderHandle {
			id: self
				.graph
				.import_resource(GraphImportedResource::Shader(desc.vs)),
		};

		let ps = GraphImportedShaderHandle {
			id: self
				.graph
				.import_resource(GraphImportedResource::Shader(desc.ps)),
		};

		let descriptor_layouts = desc.descriptor_layouts;
		let render_pass = desc.render_pass;
		let depth_write = desc.depth_write;
		let face_cull = desc.face_cull;
		let push_constant_bytes = desc.push_constant_bytes;

		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::RasterPipeline {
				name,
				vs,
				ps,
				descriptor_layouts,
				render_pass,
				depth_write,
				face_cull,
				push_constant_bytes,
			},
		);

		GraphRasterPipelineHandle { id }
	}

	pub fn add_render_pass(&mut self, desc: RenderPassDesc) -> GraphRenderPassHandle {
		let recorded = self.recorded.as_mut().unwrap();

		let name = desc.name;
		let mut color_attachments = desc.color_attachments;
		color_attachments.iter_mut().for_each(|a| {
			a.layout = ImageLayout::ColorAttachmentOptimal;
			recorded.render_attachments.push(*a)
		});

		let mut depth_attachment = desc.depth_attachment;
		if let Some(ref mut depth_attachment) = depth_attachment {
			depth_attachment.layout = ImageLayout::DepthStencilAttachmentOptimal;
			recorded.render_attachments.push(*depth_attachment);
		}

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
		let id = self
			.graph
			.create_resource(self.pass, GraphOwnedResource::OutputRenderPass {});

		GraphRenderPassHandle { id }
	}

	pub fn add_descriptor_set(&mut self, desc: DescriptorDesc<'a>) -> GraphDescriptorHandle {
		let name = desc.name;
		let bindings = desc
			.bindings
			.iter()
			.map(|(i, binding)| {
				(
					*i,
					match binding {
						DescriptorBindingDesc::ImportedBuffer(buffer) => {
							let id = self
								.graph
								.import_resource(GraphImportedResource::Buffer(buffer));
							GraphOwnedResourceDescriptorBinding::ImportedBuffer(
								GraphImportedBufferHandle { id },
							)
						}
						DescriptorBindingDesc::ImportedTexture(texture) => {
							let id = self
								.graph
								.import_resource(GraphImportedResource::Texture(texture));
							GraphOwnedResourceDescriptorBinding::ImportedTexture(
								GraphImportedTextureHandle { id },
							)
						}
						DescriptorBindingDesc::OwnedBuffer(buffer) => {
							GraphOwnedResourceDescriptorBinding::OwnedBuffer(*buffer)
						}
						DescriptorBindingDesc::Attachment(attachment) => {
							GraphOwnedResourceDescriptorBinding::Attachment(*attachment)
						}
						DescriptorBindingDesc::MutableAttachment(attachment) => {
							GraphOwnedResourceDescriptorBinding::MutableAttachment(*attachment)
						}
					},
				)
			})
			.collect::<Vec<_>>();

		// TODO(Brandon): Validate bindings with descriptor set info.

		let descriptor_layout = desc.descriptor_layout;

		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::DescriptorSet {
				name,
				bindings,
				descriptor_layout,
			},
		);

		GraphDescriptorHandle { id }
	}

	pub fn cmd_begin_render_pass(
		&mut self,
		render_pass: GraphRenderPassHandle,
		clear_value: Color,
	) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BeginRenderPass {
			render_pass,
			clear_value,
		});
	}

	pub fn cmd_bind_raster_pipeline(&mut self, pipeline: GraphRasterPipelineHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BindRasterPipeline { pipeline });
	}

	pub fn cmd_bind_raster_descriptor(
		&mut self,
		descriptor: GraphDescriptorHandle,
		set: u32,
		pipeline: GraphRasterPipelineHandle,
	) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::BindDescriptor {
			set,
			descriptor,
			pipeline,
		});
	}

	pub fn cmd_draw_mesh(&mut self, mesh: &'a Mesh) {
		let id = self
			.graph
			.import_resource(GraphImportedResource::Mesh(mesh));
		let mesh = GraphImportedMeshHandle { id };

		let recorded = self.recorded.as_mut().unwrap();
		recorded.cmds.push(PassCmd::DrawMesh { mesh });
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
