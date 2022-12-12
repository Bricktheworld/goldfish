#![allow(dead_code)]
#![allow(unused_imports)]

pub mod package;
pub mod renderer;
pub mod tracy_gpu;
pub mod types;
pub mod window;

use package::{AssetType, Package, ReadAssetFn};
use renderer::Renderer;
use std::time::Duration;
use thiserror::Error;
use tracy_client as tracy;
use uuid::Uuid;
use window::Window;

#[derive(Error, Debug)]
pub enum GoldfishError {
	#[error("A filesystem error occurred {0}")]
	Filesystem(std::io::Error),
	#[error("Unknown error {0}")]
	Unknown(String),
}

pub type GoldfishResult<T> = Result<T, GoldfishError>;

#[macro_use(defer)]
extern crate scopeguard;

pub struct GoldfishEngine {
	window: Window,
	package_reader: ReadAssetFn,
	renderer: Option<Renderer>,
	tracy: tracy::Client,
}

#[global_allocator]
static GLOBAL: tracy::ProfiledAllocator<std::alloc::System> =
	tracy::ProfiledAllocator::new(std::alloc::System, 128);

impl GoldfishEngine {
	pub fn new(title: &'static str, package_reader: ReadAssetFn) -> Self {
		let tracy = tracy::Client::start();
		let window = Window::new(title).unwrap();
		let renderer = None;

		Self {
			window,
			package_reader,
			tracy,
			renderer,
		}
	}

	pub fn read_package(&self, uuid: Uuid, asset_type: AssetType) -> GoldfishResult<Package> {
		let fn_ptr = self.package_reader;
		fn_ptr(uuid, asset_type)
	}

	pub fn run<F>(mut self, mut editor_update: F)
	where
		F: FnMut(&mut Self, Duration) + 'static,
	{
		self.renderer = Some(Renderer::new(&self.window, &self));

		Window::run(self.window.get_run_context(), move |dt, new_size| {
			tracy::span!();
			let renderer = self.renderer.as_mut().unwrap();

			if let Some(size) = new_size {
				renderer.graphics_context.on_resize(size);

				// TODO(Brandon): This is really really really fucking stupid, but it's the
				// only way I've been able to stop this ERROR_NATIVE_WINDOW_IN_USE_KHR
				// nonsense. I need to find a better solution to this
				return;
			}
			renderer.update(&self.window);

			// println!("Goldfish update {} ns", dt.as_nanos());
			editor_update(&mut self, dt);
			tracy::frame_mark();
		});
		// self.graphics_device.destroy_framebuffer(framebuffer);
	}
}

impl Drop for GoldfishEngine {
	fn drop(&mut self) {
		let renderer = self.renderer.take().unwrap();
		renderer.destroy();
	}
}
