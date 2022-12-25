include!(concat!(env!("OUT_DIR"), "/materials.rs"));

use goldfish::build::{Descriptor, UniformBuffer};
use goldfish::game::GameLib;
use goldfish::package::{AssetType, Package};
use goldfish::renderer;
use goldfish::GoldfishEngine;
use goldfish::{Color, Mat4, Quat, Vec3, Vec4};
use renderer::*;
use uuid::uuid;
use winit::event::VirtualKeyCode;

#[derive(Default, Clone, Copy)]
struct Transform {
	position: Vec3,
	rotation: Quat,
	scale: Vec3,
}

impl Transform {
	pub fn forward(&self) -> Vec3 {
		self.rotation
			* Vec3 {
				x: 0.0,
				y: 0.0,
				z: 1.0,
			}
	}

	pub fn right(&self) -> Vec3 {
		self.rotation
			* Vec3 {
				x: 1.0,
				y: 0.0,
				z: 0.0,
			}
	}

	pub fn up(&self) -> Vec3 {
		self.rotation
			* Vec3 {
				x: 0.0,
				y: 1.0,
				z: 0.0,
			}
	}
}

struct Game {
	layout_cache: DescriptorLayoutCache,
	common_desc0_heap: DescriptorHeap,
	common_desc0_layout: DescriptorLayout,
	vs: Shader,
	ps: Shader,
	cube: Mesh,
	camera_uniform: GpuBuffer,
	model_uniform: GpuBuffer,
	common_desc: DescriptorHandle,
	upload_context: UploadContext,
	pipeline_handle: OutputPipelineHandle,

	camera_transform: Transform,
	cube_transform: Transform,

	render_graph_cache: RenderGraphCache,
}

impl Game {
	fn update(&mut self, engine: &mut GoldfishEngine) {
		let graphics_device = &mut engine.graphics_device;
		let graphics_context = &mut engine.graphics_context;

		let dz = if engine.keys[VirtualKeyCode::W as usize] {
			1.0
		} else if engine.keys[VirtualKeyCode::S as usize] {
			-1.0
		} else {
			0.0
		};

		let dx = if engine.keys[VirtualKeyCode::A as usize] {
			-1.0
		} else if engine.keys[VirtualKeyCode::D as usize] {
			1.0
		} else {
			0.0
		};

		let dy = if engine.keys[VirtualKeyCode::E as usize] {
			1.0
		} else if engine.keys[VirtualKeyCode::Q as usize] {
			-1.0
		} else {
			0.0
		};

		let speed = 0.05;

		self.camera_transform.position += Vec3 {
			x: dx * speed,
			y: dy * speed,
			z: dz * speed,
		};
		let sensitivity = 0.001;
		self.camera_transform.rotation *=
			Quat::from_rotation_y(sensitivity * engine.mouse_delta.x as f32);

		self.camera_transform.rotation *= Quat::from_axis_angle(
			self.camera_transform.right(),
			sensitivity * engine.mouse_delta.y as f32,
		);

		if let Ok(_) = graphics_context.begin_frame(&engine.window) {
			let mut render_graph = RenderGraph::new(&mut self.render_graph_cache);
			let attachment_1 = {
				let mut first_pass = render_graph.add_pass("first_pass");

				first_pass.add_attachment(AttachmentDesc {
					name: "Attachment 1",
					width: engine.window.get_size().width,
					height: engine.window.get_size().height,
					format: TextureFormat::RGBA8,
					load_op: LoadOp::Clear,
					store_op: StoreOp::Store,
				})
			};
			let color_attachment = {
				let mut geometry_pass = render_graph.add_pass("geometry");

				let color_attachment = geometry_pass.add_attachment(AttachmentDesc {
					name: "Geometry Color",
					width: engine.window.get_size().width,
					height: engine.window.get_size().height,
					format: TextureFormat::RGBA8,
					load_op: LoadOp::Clear,
					store_op: StoreOp::Store,
				});

				geometry_pass.decl_read_attachment(attachment_1.into());

				let render_pass = geometry_pass.add_render_pass(RenderPassDesc {
					name: "Geometry Render Pass",
					color_attachments: vec![color_attachment],
					depth_attachment: None,
				});

				let pipeline = geometry_pass.add_raster_pipeline(RasterPipelineDesc {
					name: "Cube Pipeline",
					vs: &self.vs,
					ps: &self.ps,
					descriptor_layouts: vec![&self.common_desc0_layout],
					render_pass,
				});

				geometry_pass.cmd_bind_raster_pipeline(pipeline);

				geometry_pass.cmd_begin_render_pass(
					render_pass,
					Color {
						r: 0.0,
						g: 0.0,
						b: 0.0,
						a: 1.0,
					},
				);

				geometry_pass.cmd_end_render_pass();

				color_attachment
			};
			{
				let mut post_processing_pass = render_graph.add_pass("post_processing");

				post_processing_pass.decl_read_attachment(color_attachment.into());
				post_processing_pass.decl_read_attachment(attachment_1.into());

				let render_pass = post_processing_pass.add_output_render_pass();
				post_processing_pass.cmd_begin_render_pass(
					render_pass,
					Color {
						r: 0.0,
						g: 0.0,
						b: 0.0,
						a: 1.0,
					},
				);

				post_processing_pass.cmd_end_render_pass();
			}

			render_graph.execute(graphics_context, graphics_device);

			let model = common_inc::Model {
				matrix: Mat4::from_scale_rotation_translation(
					self.cube_transform.scale,
					self.cube_transform.rotation,
					self.cube_transform.position,
				),
			};

			let proj =
				Mat4::perspective_infinite_lh(1.8, engine.window.get_size().aspect() as f32, 0.01);

			let view = Mat4::look_at_lh(
				self.camera_transform.position,
				self.camera_transform.position + self.camera_transform.forward(),
				Vec3 {
					x: 0.0,
					y: 1.0,
					z: 0.0,
				},
			);

			let camera = common_inc::Camera {
				position: self.camera_transform.position,
				view,
				proj,
				view_proj: proj * view,
			};

			graphics_device.update_buffer(&mut self.camera_uniform, &camera.as_buffer());
			graphics_device.update_buffer(&mut self.model_uniform, &model.as_buffer());

			graphics_context.write_uniform_buffers(
				&[(0, &self.camera_uniform), (1, &self.model_uniform)],
				&self.common_desc0_heap,
				&self.common_desc,
			);
			graphics_context.bind_output_framebuffer(Color {
				r: 0.0,
				g: 0.0,
				b: 0.0,
				a: 1.0,
			});

			let pipeline = graphics_context
				.get_raster_pipeline(self.pipeline_handle)
				.unwrap();
			graphics_context.bind_raster_pipeline(pipeline);

			graphics_context.bind_graphics_descriptor(
				&self.common_desc0_heap,
				&self.common_desc,
				0,
				pipeline,
			);
			graphics_context.draw_mesh(&self.cube);

			graphics_context.unbind_output_framebuffer();
			graphics_context.end_frame(&engine.window);
		}
	}

	fn destroy(self, engine: &mut GoldfishEngine) {
		let graphics_device = &mut engine.graphics_device;
		let graphics_context = &mut engine.graphics_context;

		graphics_device.destroy_buffer(self.camera_uniform);
		graphics_device.destroy_buffer(self.model_uniform);
		graphics_device.destroy_mesh(self.cube);
		graphics_device.destroy_upload_context(self.upload_context);
		graphics_context.destroy_raster_pipeline(self.pipeline_handle);
		graphics_device.destroy_shader(self.vs);
		graphics_device.destroy_shader(self.ps);

		graphics_device.destroy_descriptor_layout_cache(self.layout_cache);
		graphics_device.destroy_descriptor_heap(self.common_desc0_heap);
	}
}

extern "C" fn on_load(engine: &mut GoldfishEngine) {
	let graphics_device = &mut engine.graphics_device;
	let graphics_context = &mut engine.graphics_context;

	let mut layout_cache = graphics_device.create_descriptor_layout_cache();

	let common_desc0_layout = graphics_device.get_graphics_layout(
		&mut layout_cache,
		DescriptorSetInfo {
			bindings: im::hashmap! {
				0 => DescriptorBindingType::CBuffer,
				1 => DescriptorBindingType::CBuffer,
			},
		},
	);

	let mut common_desc0_heap = graphics_device.create_descriptor_heap(common_desc0_layout);

	let common_desc = common_desc0_heap.alloc().unwrap();

	let vs = graphics_device.create_shader(&test_shader::VS_BYTES);
	let ps = graphics_device.create_shader(&test_shader::PS_BYTES);

	let pipeline_handle =
		graphics_context.create_raster_pipeline(&vs, &ps, &[common_desc0_layout], true, true, 0);

	let mut upload_context = graphics_device.create_upload_context();

	let camera_uniform = upload_context.create_buffer(
		common_inc::Camera::size(),
		MemoryLocation::CpuToGpu,
		BufferUsage::UniformBuffer,
		None,
		None,
	);

	let model_uniform = upload_context.create_buffer(
		common_inc::Model::size(),
		MemoryLocation::CpuToGpu,
		BufferUsage::UniformBuffer,
		None,
		None,
	);

	let Package::Mesh(mesh_package) = engine.read_package(
			uuid!("471cb8ab-2bd0-4e91-9ea9-0d0573cb9e0a"),
			AssetType::Mesh,
	      ).expect("Failed to load mesh package!") else
	      {
	          panic!("Incorrect package type loaded?");
	      };

	let cube = upload_context.create_mesh(&mesh_package.vertices, &mesh_package.indices);

	let render_graph_cache = RenderGraphCache::default();

	let game = Box::new(Game {
		layout_cache,
		common_desc0_heap,
		common_desc0_layout,
		common_desc,
		vs,
		ps,
		pipeline_handle,
		cube,
		upload_context,
		camera_uniform,
		model_uniform,
		camera_transform: Default::default(),
		cube_transform: Transform {
			position: Vec3 {
				x: 0.0,
				y: 0.0,
				z: 10.0,
			},
			scale: Vec3 {
				x: 1.0,
				y: 1.0,
				z: 1.0,
			},
			..Default::default()
		},
		render_graph_cache,
	});

	engine.game_state = Box::into_raw(game) as *mut ();
}

extern "C" fn on_unload(engine: &mut GoldfishEngine) {
	let game = unsafe { Box::from_raw(engine.game_state as *mut Game) };
	game.destroy(engine);

	engine.game_state = std::ptr::null_mut();
}

extern "C" fn on_update(engine: &mut GoldfishEngine) {
	let game = unsafe { &mut *(engine.game_state as *mut Game) };
	game.update(engine);
}

#[no_mangle]
extern "C" fn _goldfish_create_game_lib() -> GameLib {
	GameLib {
		on_load,
		on_unload,
		on_update,
	}
}
