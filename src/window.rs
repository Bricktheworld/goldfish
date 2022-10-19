use super::types::Size;
use std::{
	cell::RefCell,
	time::{Duration, Instant},
};
use winit::{
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
};

pub struct Window
{
	winit_window: winit::window::Window,
	event_loop: RefCell<Option<EventLoop<()>>>,
}

impl Window
{
	pub fn new(title: &str) -> Result<Self, winit::error::OsError>
	{
		let window_builder = winit::window::WindowBuilder::new().with_title(title);

		let event_loop = EventLoop::new();
		let winit_window = window_builder.build(&event_loop)?;

		Ok(Self {
			winit_window,
			event_loop: RefCell::new(Some(event_loop)),
		})
	}

	pub fn get_dpi(&self) -> f64
	{
		self.winit_window.scale_factor()
	}

	pub fn get_size(&self) -> Size
	{
		let size = self.winit_window.inner_size();

		Size {
			width: size.width,
			height: size.height,
		}
	}

	pub fn run<F>(mut self, update_fn: F)
	where
		F: Fn(&Self, Duration) -> () + 'static,
	{
		let event_loop = self.event_loop.take();

		if let Some(event_loop) = event_loop
		{
			let mut last_time = Instant::now();
			event_loop.run(move |event, _, control_flow| {
				*control_flow = ControlFlow::Poll;

				match event
				{
					Event::WindowEvent {
						event: WindowEvent::CloseRequested,
						..
					} => *control_flow = ControlFlow::Exit,
					Event::MainEventsCleared =>
					{
						let now = Instant::now();
						let dt = now - last_time;
						last_time = now;

						update_fn(&self, dt);
					}
					_ => (),
				}
			});
		}
		else
		{
			panic!("Cannot call 'Window::run' more than once!");
		}
	}
}
