mod renderer;
mod types;
mod window;

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
		Window::run(self.window.get_run_context(), move |dt| {
			match self.graphics_context.begin_frame(&self.window)
			{
				Ok(_) => self.graphics_context.end_frame(),
				Err(_) => (),
			}

			// println!("Goldfish update {} ns", dt.as_nanos());
			editor_update(&mut self, dt);
		});
	}
}
