mod types;
mod window;

use std::time::Duration;
use window::Window;

fn main()
{
	println!("Hello Goldfish!");

	Window::new("Goldfish")
		.unwrap()
		.run(move |window, dt| update(window, dt));
}

fn update(window: &Window, dt: Duration)
{
	println!("{} ms", dt.as_millis());
}
