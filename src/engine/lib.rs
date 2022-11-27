pub mod package;
pub mod renderer;
pub mod tracy_gpu;
pub mod types;
pub mod window;

use crate::types::Color;
use package::{AssetType, Package, ReadAssetFn};
use rand::prelude::*;
use renderer::{
	AttachmentDescription, GraphicsContext, GraphicsDevice, LoadOp, StoreOp, TextureFormat,
};
use std::time::Duration;
use thiserror::Error;
use tracy_client as tracy;
use uuid::{uuid, Uuid};
use window::Window;

#[derive(Error, Debug)]
pub enum GoldfishError
{
	#[error("A filesystem error occurred {0}")]
	Filesystem(std::io::Error),
	#[error("Unknown error {0}")]
	Unknown(String),
}

pub type GoldfishResult<T> = Result<T, GoldfishError>;

#[macro_use(defer)]
extern crate scopeguard;

pub struct GoldfishEngine
{
	window: Window,
	graphics_device: GraphicsDevice,
	graphics_context: GraphicsContext,
	package_reader: ReadAssetFn,
	tracy: tracy::Client,
}

#[global_allocator]
static GLOBAL: tracy::ProfiledAllocator<std::alloc::System> =
	tracy::ProfiledAllocator::new(std::alloc::System, 128);

impl GoldfishEngine
{
	pub fn new(title: &'static str, package_reader: ReadAssetFn) -> Self
	{
		let tracy = tracy::Client::start();
		let window = Window::new(title).unwrap();
		let (graphics_device, graphics_context) = GraphicsDevice::new_with_context(&window);

		Self {
			window,
			graphics_device,
			graphics_context,
			package_reader,
			tracy,
		}
	}

	pub fn read_package(&self, uuid: Uuid, asset_type: AssetType) -> GoldfishResult<Package>
	{
		let fn_ptr = self.package_reader;
		fn_ptr(uuid, asset_type)
	}

	pub fn run<F>(mut self, mut editor_update: F)
	where
		F: FnMut(&mut Self, Duration) + 'static,
	{
		let Package::Shader(shader_package) = self.read_package(
			uuid!("07060963-a7eb-49a3-91c0-9e5b453773ee"),
			AssetType::Shader,
		).expect("Failed to load shader package!") else
		{
            panic!("Incorrect package type loaded?");
		};

		let vertex_shader = self
			.graphics_device
			.create_shader(&shader_package.vs_ir.expect("No vertex shader!"));

		let pixel_shader = self
			.graphics_device
			.create_shader(&shader_package.ps_ir.expect("No vertex shader!"));

		let render_pass = self.graphics_device.create_render_pass(
			&[AttachmentDescription {
				format: TextureFormat::RGBA8,
				load_op: LoadOp::Clear,
				store_op: StoreOp::Store,
			}],
			None,
		);

		let pipeline = self.graphics_device.create_raster_pipeline(
			&vertex_shader,
			&pixel_shader,
			&render_pass,
			true,
			true,
			0,
		);

		self.graphics_device.destroy_raster_pipeline(pipeline);
		self.graphics_device.destroy_shader(vertex_shader);
		self.graphics_device.destroy_shader(pixel_shader);
		self.graphics_device.destroy_render_pass(render_pass);

		let mut rng = rand::thread_rng();
		Window::run(self.window.get_run_context(), move |dt, new_size| {
			tracy::span!();
			if let Some(size) = new_size
			{
				self.graphics_context.on_resize(size);

				// TODO(Brandon): This is really really really fucking stupid, but it's the
				// only way I've been able to stop this ERROR_NATIVE_WINDOW_IN_USE_KHR
				// nonsense. I need to find a better solution to this
				return;
			}

			if let Ok(_) = self.graphics_context.begin_frame(&self.window)
			{
				self.graphics_context.bind_output_framebuffer(Color {
					r: rng.gen(),
					g: rng.gen(),
					b: rng.gen(),
					a: 1.0,
				});
				self.graphics_context.unbind_output_framebuffer();
				self.graphics_context.end_frame(&self.window);
			}

			// println!("Goldfish update {} ns", dt.as_nanos());
			editor_update(&mut self, dt);
			tracy::frame_mark();
		});
	}
}

impl Drop for GoldfishEngine
{
	fn drop(&mut self)
	{
		self.graphics_context.destroy();
		self.graphics_device.destroy();
	}
}
