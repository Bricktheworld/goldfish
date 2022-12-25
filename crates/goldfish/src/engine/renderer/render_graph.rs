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
	// BindPipeline(vk::PipelineBindPoint, vk::Pipeline),
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

#[derive(Default)]
pub struct BufferCache {}

#[derive(Default)]
pub struct TextureCache {}

#[derive(Default)]
pub struct AttachmentCache {}

#[derive(Default)]
pub struct FramebufferCache {}

#[derive(Default)]
pub struct RenderPassCache {
	pub cache: HashMap<RenderPassDesc, RenderPass>,
}

#[derive(Default)]
pub struct PipelineCache {}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct AttachmentDesc {
	pub name: &'static str,
	pub width: u32,
	pub height: u32,
	pub format: TextureFormat,
	pub load_op: LoadOp,
	pub store_op: StoreOp,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct FramebufferDesc {
	pub width: u32,
	pub height: u32,
}

#[derive(Clone, Hash, PartialEq, Eq)]
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
	pub descriptor_layouts: Vec<&'a DescriptorLayout>,
	pub render_pass: GraphRenderPassHandle,
}

#[derive(Debug, Clone)]
pub struct RecordedPass {
	pub name: &'static str,
	pass: PassHandle,
	pub cmds: Vec<PassCmd>,
	pub read_attachments: Vec<GraphAttachmentHandle>,
	pub write_attachments: Vec<MutableGraphAttachmentHandle>,
}

#[derive(Default)]
pub struct RenderGraphCache {
	pub buffer_cache: BufferCache,
	pub texture_cache: TextureCache,
	pub attachment_cache: AttachmentCache,
	pub framebuffer_cache: FramebufferCache,
	pub render_pass_cache: RenderPassCache,
	pub pipeline_cache: PipelineCache,
}

#[derive(Clone, Copy)]
pub enum GraphImportedResource<'a> {
	Shader(&'a Shader),
	Mesh(&'a Mesh),
	DescriptorLayout(&'a DescriptorLayout),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedShaderHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphImportedDescriptorLayoutHandle {
	id: usize,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum GraphOwnedResource {
	RasterPipeline {
		name: &'static str,
		vs: GraphImportedShaderHandle,
		ps: GraphImportedShaderHandle,
		descriptor_layouts: Vec<GraphImportedDescriptorLayoutHandle>,
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
		usage: ash::vk::ImageUsageFlags, // TODO(Brandon): Make this backend agnostic
		layout: ash::vk::ImageLayout,    // TODO(Brandon): Make this backend agnostic
		load_op: LoadOp,
		store_op: StoreOp,
	},
	Buffer {
		name: &'static str,
		size: usize,
		usage: ash::vk::BufferUsageFlags, // TODO(Brandon): Make this backend agnostic
	},
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphRasterPipelineHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct GraphAttachmentHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MutableGraphAttachmentHandle {
	id: usize,
}

impl GraphAttachmentHandle {
	fn upgrade(self) -> MutableGraphAttachmentHandle {
		MutableGraphAttachmentHandle { id: self.id }
	}
}

impl From<MutableGraphAttachmentHandle> for GraphAttachmentHandle {
	fn from(mutable: MutableGraphAttachmentHandle) -> Self {
		Self { id: mutable.id }
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GraphRenderPassHandle {
	id: usize,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct PassHandle {
	id: usize,
}

pub struct RenderGraph<'a> {
	pub passes: Vec<RecordedPass>,
	pub owned_resources: Vec<GraphOwnedResource>,
	resource_to_owning_pass: HashMap<usize, PassHandle>,
	pub imported_resources: Vec<GraphImportedResource<'a>>,
	pub cache: &'a mut RenderGraphCache,
}

#[derive(Debug)]
struct PassDependencyNode {
	pass: PassHandle,
	dependencies: Vec<PassDependencyNode>,
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

	fn allocate_resources(&mut self) {
		for resource in self.owned_resources.iter() {}
	}

	pub fn execute(mut self, context: &mut GraphicsContext, device: &mut GraphicsDevice) {
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
		let render_pass_output_handle = GraphRenderPassHandle { id };

		let root_pass = self.resource_to_owning_pass[&id];
		let mut pass_order = vec![root_pass];
		let root = self.resolve_pass_dependencies(root_pass, &mut pass_order);

		{
			let mut found = HashSet::<PassHandle>::new();
			pass_order.reverse();
			pass_order.retain(|p| found.insert(*p));
		}

		dbg!(&pass_order);
		dbg!(&root);

		self.allocate_resources();

		// for pass in self.passes {
		// 	for cmd in pass.cmds {
		// 		match cmd {
		// 			PassCmd::BeginRenderPass {
		//                       ..
		// 				// render_pass,
		// 				// clear_value,
		// 			} => todo!(),
		// 			PassCmd::EndRenderPass {} => {
		// 				context.end_render_pass();
		// 			}
		// 			// PassCmd::BeginOutputRenderPass { clear_value } => {
		// 			// 	context.bind_output_framebuffer(clear_value);
		// 			// }
		//                   PassCmd::BindRasterPipeline {pipeline} => {
		//                       todo!()
		//                       // context.bind_raster_pipeline();
		//                   }
		// 		}
		// 	}
		// }
	}

	fn import_resource(&mut self, resource: GraphImportedResource<'a>) -> usize {
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
	pub graph: &'b mut RenderGraph<'a>,
	pass: PassHandle,
	pub recorded: Option<RecordedPass>,
}

impl<'a, 'b> PassBuilder<'a, 'b> {
	pub fn add_attachment(
		&mut self,
		desc: AttachmentDesc,
		// layout: ash::vk::ImageLayout,
		// usage: ash::vk::ImageUsageFlags,
	) -> MutableGraphAttachmentHandle {
		let id = self.graph.create_resource(
			self.pass,
			GraphOwnedResource::Attachment {
				name: desc.name,
				width: desc.width,
				height: desc.height,
				format: desc.format,
				load_op: desc.load_op,
				store_op: desc.store_op,
				layout: ash::vk::ImageLayout::GENERAL,
				usage: ash::vk::ImageUsageFlags::COLOR_ATTACHMENT, // TODO(Brandon): Not even remotely close to how we're supposed to do this
			},
		);
		MutableGraphAttachmentHandle { id }
	}

	pub fn decl_read_attachment(&mut self, attachment: GraphAttachmentHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.read_attachments.push(attachment);
	}

	pub fn decl_write_attachment(&mut self, attachment: MutableGraphAttachmentHandle) {
		let recorded = self.recorded.as_mut().unwrap();
		recorded.write_attachments.push(attachment);
	}

	pub fn add_raster_pipeline(
		&mut self,
		desc: RasterPipelineDesc<'a>,
	) -> GraphRasterPipelineHandle {
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

		let descriptor_layouts = desc
			.descriptor_layouts
			.into_iter()
			.map(|descriptor_layout| GraphImportedDescriptorLayoutHandle {
				id: self
					.graph
					.import_resource(GraphImportedResource::DescriptorLayout(descriptor_layout)),
			})
			.collect::<Vec<_>>();

		let name = desc.name;

		let id = self.graph.owned_resources.len();

		self.graph.create_resource(
			self.pass,
			GraphOwnedResource::RasterPipeline {
				name,
				vs,
				ps,
				descriptor_layouts,
			},
		);

		GraphRasterPipelineHandle { id }
	}

	pub fn add_render_pass(&mut self, desc: RenderPassDesc) -> GraphRenderPassHandle {
		let id = self.graph.owned_resources.len();
		let name = desc.name;
		let color_attachments = desc.color_attachments;
		let depth_attachment = desc.depth_attachment;

		self.graph.create_resource(
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
		let id = self.graph.owned_resources.len();

		self.graph
			.create_resource(self.pass, GraphOwnedResource::OutputRenderPass {});

		GraphRenderPassHandle { id }
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
