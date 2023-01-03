include!(concat!(env!("OUT_DIR"), "/materials.rs"));

use glam::Vec4Swizzles;
use goldfish::build::{CBuffer, StructuredBuffer};
use goldfish::game::GameLib;
use goldfish::package::{AssetType, Package};
use goldfish::renderer;
use goldfish::GoldfishEngine;
use goldfish::{Mat4, Quat, UVec2, Vec3, Vec4};
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
		self.rotation * Vec3 { x: 0.0, y: 0.0, z: 1.0 }
	}

	pub fn right(&self) -> Vec3 {
		self.rotation * Vec3 { x: 1.0, y: 0.0, z: 0.0 }
	}

	pub fn up(&self) -> Vec3 {
		self.rotation * Vec3 { x: 0.0, y: 1.0, z: 0.0 }
	}
}

const COMMON_DESC_INFO: &'static DescriptorSetInfo = &DescriptorSetInfo {
	bindings: phf::phf_map! {
		0u32 => DescriptorBindingType::CBuffer,
		1u32 => DescriptorBindingType::CBuffer,
	},
};

const SAMPLER_DESC_INFO: &'static DescriptorSetInfo = &DescriptorSetInfo {
	bindings: phf::phf_map! {
		0u32 => DescriptorBindingType::Texture2D,
		1u32 => DescriptorBindingType::SamplerState,
	},
};

const FULLSCREEN_DESC_INFO: &'static DescriptorSetInfo = &DescriptorSetInfo {
	bindings: phf::phf_map! {
		0u32 => DescriptorBindingType::Texture2D,
		1u32 => DescriptorBindingType::SamplerState,
	},
};

const DEPTH_DESC_INFO: &'static DescriptorSetInfo = &DescriptorSetInfo {
	bindings: phf::phf_map! {
		0u32 => DescriptorBindingType::Texture2D,
		1u32 => DescriptorBindingType::SamplerState,
		2u32 => DescriptorBindingType::CBuffer,
	},
};

const LIGHT_CULL_DESC_INFO: &'static DescriptorSetInfo = &DescriptorSetInfo {
	bindings: phf::phf_map! {
		0u32 => DescriptorBindingType::StructuredBuffer,
		1u32 => DescriptorBindingType::CBuffer,
		2u32 => DescriptorBindingType::Texture2D,
		3u32 => DescriptorBindingType::RWTexture2D,
	},
};

const Z_NEAR: f32 = 0.01;

struct Game {
	vs: Shader,
	ps: Shader,
	vs_textured: Shader,
	ps_textured: Shader,
	vs_fullscreen: Shader,
	ps_fullscreen: Shader,
	ps_depth_debug: Shader,
	cs_light_cull: Shader,
	point_lights: [light_cull_compute::PointLight; 3],
	point_lights_sbuffer: GpuBuffer,
	light_cull_cbuffer: GpuBuffer,
	depth_debug_cbuffer: GpuBuffer,
	cube: Mesh,
	camera_uniform: GpuBuffer,
	model_uniform: GpuBuffer,
	upload_context: UploadContext,

	camera_transform: Transform,
	camera_heading: f64,
	camera_pitch: f64,
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

		let sensitivity = 0.001;
		self.camera_pitch += sensitivity * engine.mouse_delta.y as f64;
		self.camera_pitch = self.camera_pitch.clamp(-std::f64::consts::FRAC_PI_2 + 0.001, std::f64::consts::FRAC_PI_2 - 0.001);
		self.camera_heading += sensitivity * engine.mouse_delta.x as f64;
		let new_rot = Quat::from_euler(glam::EulerRot::YXZ, self.camera_heading as f32, self.camera_pitch as f32, 0.0);
		self.camera_transform.rotation = self.camera_transform.rotation.slerp(new_rot, 0.3);

		let speed = 0.05;
		self.camera_transform.position += speed * (self.camera_transform.forward() * dz + self.camera_transform.right() * dx + Vec3 { x: 0.0, y: 1.0, z: 0.0 } * dy);

		if let Ok(_) = graphics_context.begin_frame(&engine.window) {
			let model = common_inc::Model {
				matrix: Mat4::from_scale_rotation_translation(self.cube_transform.scale, self.cube_transform.rotation, self.cube_transform.position),
			};

			let proj = Mat4::perspective_infinite_reverse_lh(1.6, engine.window.get_size().aspect() as f32, Z_NEAR);
			let inverse_proj = proj.inverse();

			let view = Mat4::look_at_lh(
				self.camera_transform.position,
				self.camera_transform.position + self.camera_transform.forward(),
				Vec3 { x: 0.0, y: 1.0, z: 0.0 },
			);

			dbg!("View matrix {}", view);
			let camera = common_inc::Camera {
				position: self.camera_transform.position,
				view,
				proj,
				view_proj: proj * view,
			};

			graphics_device.update_buffer(&mut self.camera_uniform, &camera.as_buffer());
			graphics_device.update_buffer(&mut self.model_uniform, &model.as_buffer());
			graphics_device.update_buffer(
				&mut self.light_cull_cbuffer,
				&light_cull_compute::CullInfo {
					screen_size: UVec2::new(engine.window.get_size().width, engine.window.get_size().height),
					view,
					z_near: Z_NEAR,
					inverse_proj,
					proj,
					light_count: 1,
				}
				.as_buffer(),
			);

			let deltas = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
			for (i, point_light) in self.point_lights.iter_mut().enumerate() {
				let view_pos = view * Vec4::new(2.0, 0.0, 0.0, 1.0);
				point_light.position = Vec3::new(2.0, 0.0, 0.0);
				let point_on_light_radius = Vec4::new(2.0, 2.0, 0.0, 1.0);
				let view_point_on_light_radius = view * point_on_light_radius;
				let view_radius = view_point_on_light_radius.xyz().distance(point_light.position);
				point_light.radius = 2.0;
			}

			{
				let dst = graphics_device.get_buffer_dst(&mut self.point_lights_sbuffer);
				light_cull_compute::PointLight::copy_to_raw(&self.point_lights, dst);
			}

			let mut render_graph = RenderGraph::new(&mut self.render_graph_cache);
			let depth_prepass_attachment = {
				let mut geometry_pass = render_graph.add_pass("geometry");

				// let mut output = geometry_pass.add_attachment(AttachmentDesc {
				// 	name: "Geometry output",
				// 	format: TextureFormat::RGBA8,
				// 	width: engine.window.get_size().width,
				// 	height: engine.window.get_size().height,
				// 	load_op: LoadOp::Clear,
				// 	store_op: StoreOp::Store,
				// 	usage: TextureUsage::SAMPLED | TextureUsage::ATTACHMENT,
				// });

				let mut depth = geometry_pass.add_attachment(AttachmentDesc {
					name: "Geometry depth",
					format: TextureFormat::Depth,
					width: engine.window.get_size().width,
					height: engine.window.get_size().height,
					load_op: LoadOp::Clear,
					store_op: StoreOp::Store,
					usage: TextureUsage::SAMPLED | TextureUsage::ATTACHMENT,
				});

				let descriptor = geometry_pass.add_graphics_descriptor_set(DescriptorDesc {
					name: "Geometry descriptor",
					descriptor_layout: COMMON_DESC_INFO,
					bindings: &mut [
						(0, DescriptorBindingDesc::ImportedBuffer(&self.camera_uniform)),
						(1, DescriptorBindingDesc::ImportedBuffer(&self.model_uniform)),
					],
				});

				let render_pass = geometry_pass.add_render_pass(RenderPassDesc {
					name: "Geometry render pass",
					color_attachments: &mut [],
					depth_attachment: Some(&mut depth),
				});

				let pipeline = geometry_pass.add_raster_pipeline(RasterPipelineDesc {
					name: "Cube Pipeline",
					vs: &self.vs,
					ps: None,
					descriptor_layouts: &[COMMON_DESC_INFO],
					render_pass,
					depth_compare_op: Some(DepthCompareOp::Greater),
					depth_write: true,
					face_cull: FaceCullMode::Back,
					push_constant_bytes: 0,
					vertex_input_info: Vertex::VERTEX_INFO,
					polygon_mode: PolygonMode::Fill,
				});

				geometry_pass.cmd_begin_render_pass(render_pass, &[ClearValue::DepthStencil { depth: 0.0, stencil: 0 }]);

				geometry_pass.cmd_bind_raster_pipeline(pipeline);
				geometry_pass.cmd_bind_graphics_descriptor(descriptor, 0, pipeline);

				geometry_pass.cmd_draw_mesh(&self.cube);

				geometry_pass.cmd_end_render_pass();

				depth
			};

			let cull_attachment = {
				let mut cull_pass = render_graph.add_pass("cull");

				let mut max_depth = cull_pass.add_attachment(AttachmentDesc {
					name: "Max Depth",
					format: TextureFormat::RGBA8UNorm,
					width: engine.window.get_size().width,
					height: engine.window.get_size().height,
					load_op: LoadOp::Clear,
					store_op: StoreOp::Store,
					usage: TextureUsage::SAMPLED | TextureUsage::STORAGE,
				});

				let descriptor = cull_pass.add_compute_descriptor_set(DescriptorDesc {
					name: "Cull Descriptor",
					descriptor_layout: LIGHT_CULL_DESC_INFO,
					bindings: &mut [
						(0, DescriptorBindingDesc::ImportedBuffer(&self.point_lights_sbuffer)),
						(1, DescriptorBindingDesc::ImportedBuffer(&self.light_cull_cbuffer)),
						(2, DescriptorBindingDesc::Attachment(depth_prepass_attachment.read())),
						(3, DescriptorBindingDesc::MutableAttachment(&mut max_depth)),
					],
				});

				let pipeline = cull_pass.add_compute_pipeline(ComputePipelineDesc {
					name: "Cull Pipeline",
					cs: &self.cs_light_cull,
					descriptor_layouts: &[LIGHT_CULL_DESC_INFO],
				});

				cull_pass.cmd_bind_compute_pipeline(pipeline);
				cull_pass.cmd_bind_compute_descriptor(descriptor, 0, pipeline);
				let work_groups_x = (engine.window.get_size().width + (engine.window.get_size().width % 16)) / 16;
				let work_groups_y = (engine.window.get_size().height + (engine.window.get_size().height % 16)) / 16;
				cull_pass.cmd_dispatch(work_groups_x, work_groups_y, 1);

				max_depth
			};
			// {
			// 	let mut sampler_pass = render_graph.add_pass("sampler pass");

			// 	let descriptor0 = sampler_pass.add_descriptor_set(DescriptorDesc {
			// 		name: "Geometry descriptor",
			// 		descriptor_layout: COMMON_DESC_INFO,
			// 		bindings: &mut [
			// 			(0, DescriptorBindingDesc::ImportedBuffer(&self.camera_uniform)),
			// 			(1, DescriptorBindingDesc::ImportedBuffer(&self.model_uniform)),
			// 		],
			// 	});

			// 	let descriptor1 = sampler_pass.add_descriptor_set(DescriptorDesc {
			// 		name: "Sampler descriptor",
			// 		descriptor_layout: SAMPLER_DESC_INFO,
			// 		bindings: &mut [
			// 			(0, DescriptorBindingDesc::Attachment(geometry_output_attachment.read())),
			// 			(1, DescriptorBindingDesc::Attachment(geometry_output_attachment.read())),
			// 		],
			// 	});

			// 	let render_pass = sampler_pass.add_output_render_pass();

			// 	let pipeline = sampler_pass.add_raster_pipeline(RasterPipelineDesc {
			// 		name: "Sampler Cube Pipeline",
			// 		vs: &self.vs_textured,
			// 		ps: &self.ps_textured,
			// 		descriptor_layouts: &[COMMON_DESC_INFO, SAMPLER_DESC_INFO],
			// 		render_pass,
			// 		depth_write: true,
			// 		face_cull: FaceCullMode::Back,
			// 		push_constant_bytes: 0,
			// 		vertex_input_info: Vertex::VERTEX_INFO,
			// 		polygon_mode: PolygonMode::Fill,
			// 	});

			// 	sampler_pass.cmd_begin_render_pass(render_pass, &[ClearValue::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }]);

			// 	sampler_pass.cmd_bind_raster_pipeline(pipeline);
			// 	sampler_pass.cmd_bind_raster_descriptor(descriptor0, 0, pipeline);
			// 	sampler_pass.cmd_bind_raster_descriptor(descriptor1, 1, pipeline);

			// 	sampler_pass.cmd_draw_mesh(&self.cube);

			// 	sampler_pass.cmd_end_render_pass();
			// }
			{
				let mut fullscreen = render_graph.add_pass("fullscreen");

				let render_pass = fullscreen.add_output_render_pass();

				let pipeline = fullscreen.add_raster_pipeline(RasterPipelineDesc {
					name: "Fullscreen Pipeline",
					vs: &self.vs_fullscreen,
					ps: Some(&self.ps_fullscreen),
					descriptor_layouts: &[FULLSCREEN_DESC_INFO],
					render_pass,
					depth_compare_op: None,
					depth_write: false,
					face_cull: FaceCullMode::Front,
					push_constant_bytes: 0,
					vertex_input_info: EMPTY_VERTEX_INFO,
					polygon_mode: PolygonMode::Fill,
				});

				let descriptor0 = fullscreen.add_graphics_descriptor_set(DescriptorDesc {
					name: "Fullscreen Descriptor",
					descriptor_layout: FULLSCREEN_DESC_INFO,
					bindings: &mut [
						(0, DescriptorBindingDesc::Attachment(cull_attachment.read())),
						(1, DescriptorBindingDesc::Attachment(cull_attachment.read())),
					],
				});

				fullscreen.cmd_begin_render_pass(render_pass, &[ClearValue::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }]);

				fullscreen.cmd_bind_raster_pipeline(pipeline);
				fullscreen.cmd_bind_graphics_descriptor(descriptor0, 0, pipeline);
				fullscreen.cmd_draw(3, 1, 0, 0);

				fullscreen.cmd_end_render_pass();
			}

			render_graph.execute(graphics_context, graphics_device);

			graphics_context.end_frame(&engine.window);
		}
	}

	fn destroy(self, engine: &mut GoldfishEngine) {
		let graphics_device = &mut engine.graphics_device;
		self.render_graph_cache.destroy(graphics_device);

		graphics_device.destroy_buffer(self.light_cull_cbuffer);
		graphics_device.destroy_buffer(self.camera_uniform);
		graphics_device.destroy_buffer(self.model_uniform);
		graphics_device.destroy_buffer(self.depth_debug_cbuffer);
		graphics_device.destroy_mesh(self.cube);
		graphics_device.destroy_upload_context(self.upload_context);
		graphics_device.destroy_shader(self.vs);
		graphics_device.destroy_shader(self.ps);
		graphics_device.destroy_shader(self.vs_textured);
		graphics_device.destroy_shader(self.ps_textured);
		graphics_device.destroy_shader(self.vs_fullscreen);
		graphics_device.destroy_shader(self.ps_fullscreen);
		graphics_device.destroy_shader(self.ps_depth_debug);
		graphics_device.destroy_shader(self.cs_light_cull);
	}
}

extern "C" fn on_load(engine: &mut GoldfishEngine) {
	let graphics_device = &mut engine.graphics_device;

	let vs = graphics_device.create_shader(&test_shader::VS_BYTES);
	let ps = graphics_device.create_shader(&test_shader::PS_BYTES);

	let vs_textured = graphics_device.create_shader(&test_sampler::VS_BYTES);
	let ps_textured = graphics_device.create_shader(&test_sampler::PS_BYTES);

	let vs_fullscreen = graphics_device.create_shader(&fullscreen::VS_BYTES);
	let ps_fullscreen = graphics_device.create_shader(&fullscreen::PS_BYTES);

	let ps_depth_debug = graphics_device.create_shader(&debug_depth::PS_BYTES);

	let cs_light_cull = graphics_device.create_shader(&light_cull_compute::CS_BYTES);

	let mut upload_context = graphics_device.create_upload_context();

	let camera_uniform = upload_context.create_buffer(common_inc::Camera::size(), MemoryLocation::CpuToGpu, BufferUsage::UniformBuffer, None, None);

	let model_uniform = upload_context.create_buffer(common_inc::Model::size(), MemoryLocation::CpuToGpu, BufferUsage::UniformBuffer, None, None);

	let depth_debug_cbuffer = upload_context.create_buffer(
		debug_depth::NearPlane::size(),
		MemoryLocation::CpuToGpu,
		BufferUsage::UniformBuffer,
		None,
		Some(&debug_depth::NearPlane { z_near: Z_NEAR, z_scale: 0.02 }.as_buffer()),
	);

	let light_cull_cbuffer = upload_context.create_buffer(light_cull_compute::CullInfo::size(), MemoryLocation::CpuToGpu, BufferUsage::UniformBuffer, None, None);
	let point_lights_sbuffer = upload_context.create_buffer(light_cull_compute::PointLight::size() * 3, MemoryLocation::CpuToGpu, BufferUsage::StorageBuffer, None, None);

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
		vs,
		ps,
		vs_textured,
		ps_textured,
		vs_fullscreen,
		ps_fullscreen,
		ps_depth_debug,
		cs_light_cull,

		light_cull_cbuffer,
		point_lights: Default::default(),
		point_lights_sbuffer,

		depth_debug_cbuffer,
		cube,
		upload_context,
		camera_uniform,
		model_uniform,
		camera_transform: Transform {
			position: Vec3 { x: 0.0, y: 0.0, z: -1.0 },
			..Default::default()
		},
		camera_heading: 0.0,
		camera_pitch: 0.0,
		cube_transform: Transform {
			position: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
			scale: Vec3 { x: 1.0, y: 1.0, z: 1.0 },
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
	GameLib { on_load, on_unload, on_update }
}
