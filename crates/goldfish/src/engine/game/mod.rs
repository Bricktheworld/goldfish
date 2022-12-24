use crate::GoldfishEngine;
use std::time::Duration;

#[repr(C)]
pub struct GameLib {
	pub on_load: extern "C" fn(&mut GoldfishEngine),
	pub on_unload: extern "C" fn(&mut GoldfishEngine),
	pub on_update: extern "C" fn(&mut GoldfishEngine),
	// setup: fn(),
	// destroy: fn(),
}

pub type CreateGamelibApi = unsafe fn() -> GameLib;
