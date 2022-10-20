mod renderer;
mod types;
mod window;

use renderer::GraphicsDevice;
use std::time::Duration;
use window::Window;

fn main()
{
	println!("Hello Goldfish!");

	let window = Window::new("Goldfish").unwrap();

	let graphics_device = GraphicsDevice::new(&window);
	window.run(move |dt| update(&graphics_device, dt));
}

fn update(graphics_device: &GraphicsDevice, dt: Duration)
{
	println!("{} ms", dt.as_millis());
}
