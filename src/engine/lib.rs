mod renderer;
mod types;
mod window;

use crate::types::Color;
use renderer::{GraphicsContext, GraphicsDevice};
use std::time::Duration;
use window::Window;

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
		let (graphics_device, graphics_context) = GraphicsDevice::new(&window);

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
					r: 0.0,
					g: 0.0,
					b: 0.0,
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
