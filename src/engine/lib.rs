pub mod renderer;
pub mod types;
pub mod window;

use crate::types::Color;
use rand::prelude::*;
use renderer::{GraphicsContext, GraphicsDevice};
use std::time::Duration;
use window::Window;

#[macro_use(defer)]
extern crate scopeguard;

pub struct GoldfishEngine
{
	window: Window,
	graphics_device: GraphicsDevice,
	graphics_context: GraphicsContext,
}

impl GoldfishEngine
{
	pub fn new(title: &'static str) -> Self
	{
		let window = Window::new(title).unwrap();
		let (graphics_device, graphics_context) = GraphicsDevice::new_with_context(&window);

		Self {
			window,
			graphics_device,
			graphics_context,
		}
	}

	pub fn run<F>(mut self, mut editor_update: F)
	where
		F: FnMut(&mut Self, Duration) + 'static,
	{
		let mut rng = rand::thread_rng();
		Window::run(self.window.get_run_context(), move |dt, new_size| {
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
