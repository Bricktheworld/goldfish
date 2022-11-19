use crate::types::Size;
use raw_window_handle::HasRawDisplayHandle;
use std::time::{Duration, Instant};
use winit::{
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
};

pub struct Window
{
	name: &'static str,
	winit_window: winit::window::Window,
	event_loop: Option<EventLoop<()>>,
}

pub type WindowRunContext = EventLoop<()>;

impl Window
{
	pub fn new(name: &'static str) -> Result<Self, winit::error::OsError>
	{
		let window_builder = winit::window::WindowBuilder::new().with_title(name);

		let event_loop = EventLoop::new();
		let winit_window = window_builder.build(&event_loop)?;
		winit_window.raw_display_handle();

		Ok(Self {
			name,
			winit_window,
			event_loop: Some(event_loop),
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

	pub fn get_winit(&self) -> &winit::window::Window
	{
		&self.winit_window
	}

	pub fn get_name(&self) -> &'static str
	{
		self.name
	}

	pub fn get_run_context(&mut self) -> WindowRunContext
	{
		self.event_loop
			.take()
			.expect("Cannot get call get_run_context more than once!")
	}

	pub fn run<F>(context: WindowRunContext, mut update_fn: F)
	where
		F: FnMut(Duration, Option<Size>) -> () + 'static,
	{
		let event_loop = context;

		let mut last_time = Instant::now();
		let mut new_size: Option<Size> = None;
		event_loop.run(move |event, _, control_flow| {
			*control_flow = ControlFlow::Poll;

			match event
			{
				Event::WindowEvent {
					event: WindowEvent::CloseRequested,
					..
				} => *control_flow = ControlFlow::Exit,
				Event::WindowEvent {
					event: WindowEvent::Resized(size),
					..
				} =>
				{
					new_size = Some(Size {
						width: size.width,
						height: size.height,
					})
				}
				Event::MainEventsCleared =>
				{
					let now = Instant::now();
					let dt = now - last_time;
					last_time = now;

					update_fn(dt, new_size);
					new_size = None;
				}
				_ => (),
			}
		});
	}
}
