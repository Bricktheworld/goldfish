#![allow(dead_code)]
#![allow(unused_imports)]

pub mod build;
pub mod game;
pub mod package;
pub mod renderer;
pub mod tracy_gpu;
pub mod types;
pub mod window;

pub use glam::*;
use package::{AssetType, Package, ReadAssetFn};
use renderer::{GraphicsContext, GraphicsDevice};
use std::time::Duration;
use thiserror::Error;
use tracy_client as tracy;
pub use types::*;
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
	pub window: Window,
	package_reader: ReadAssetFn,
	pub graphics_device: GraphicsDevice,
	pub graphics_context: GraphicsContext,
	pub game_state: *mut (),
	tracy: tracy::Client,
	pub keys: [bool; 255],
	pub mouse_delta: DVec2,
}

#[global_allocator]
static GLOBAL: tracy::ProfiledAllocator<std::alloc::System> =
	tracy::ProfiledAllocator::new(std::alloc::System, 128);

impl GoldfishEngine {
	pub fn new(title: &'static str, package_reader: ReadAssetFn) -> Self {
		let tracy = tracy::Client::start();
		let window = Window::new(title).unwrap();
		let game_state = std::ptr::null_mut();
		let keys = [false; 255];
		let mouse_delta = Default::default();

		let (graphics_device, graphics_context) = GraphicsDevice::new_with_context(&window);

		Self {
			window,
			graphics_device,
			graphics_context,
			package_reader,
			tracy,
			game_state,
			keys,
			mouse_delta,
		}
	}

	pub fn read_package(&self, uuid: Uuid, asset_type: AssetType) -> GoldfishResult<Package> {
		let fn_ptr = self.package_reader;
		fn_ptr(uuid, asset_type)
	}

	pub fn run<F>(&mut self, mut editor_update: F)
	where
		F: FnMut(&mut Self, Duration),
	{
		Window::run(
			self.window.get_run_context(),
			|dt, keys, mouse_delta, new_size| {
				self.keys.copy_from_slice(keys);
				self.mouse_delta = mouse_delta;

				tracy::span!();
				// let renderer = self.renderer.as_mut().unwrap();

				if let Some(size) = new_size {
					self.graphics_context.on_resize(size);

					// TODO(Brandon): This is really really really fucking stupid, but it's the
					// only way I've been able to stop this ERROR_NATIVE_WINDOW_IN_USE_KHR
					// nonsense. I need to find a better solution to this
					return;
				}
				// renderer.update(&self.window);

				editor_update(self, dt);
				tracy::frame_mark();
			},
		);
	}

	pub fn lock_cursor(&self) {
		self.window
			.winit_window
			.set_cursor_grab(winit::window::CursorGrabMode::Locked)
			.unwrap();
		self.window.winit_window.set_cursor_visible(false);
	}

	pub fn unlock_cursor(&self) {
		self.window.winit_window.set_cursor_visible(true);
		self.window
			.winit_window
			.set_cursor_grab(winit::window::CursorGrabMode::None)
			.unwrap();
	}
}

impl Drop for GoldfishEngine {
	fn drop(&mut self) {
		// let renderer = self.renderer.take().unwrap();
		// renderer.destroy();
		self.graphics_context.destroy();
		self.graphics_device.destroy();
	}
}
