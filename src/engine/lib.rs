mod renderer;
mod types;
mod window;

use renderer::GraphicsDevice;
use std::time::Duration;
use window::Window;

pub struct GoldfishEngine
{
	window: Window,
	graphics_device: GraphicsDevice,
}

impl GoldfishEngine
{
	pub fn new(title: &'static str) -> Self
	{
		let window = Window::new(title).unwrap();
		let graphics_device = GraphicsDevice::new(&window);

		Self {
			window,
			graphics_device,
		}
	}

	pub fn run<F>(&self, editor_update: F)
	where
		F: Fn(Duration) + 'static,
	{
		self.window.run(move |dt| {
			println!("Goldfish update {} ns", dt.as_nanos());
			editor_update(dt);
		});
	}
}
