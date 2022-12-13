#[repr(C)]
pub struct GameLib {
	pub on_load: extern "C" fn(),
	pub on_unload: extern "C" fn(),
	// setup: fn(),
	// destroy: fn(),
}

pub type CreateGamelibApi = unsafe fn() -> GameLib;
