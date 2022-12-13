include!(concat!(env!("OUT_DIR"), "/materials.rs"));

use goldfish::build::DescriptorInfo;
use goldfish::game::GameLib;

fn main() {
	let material = test_shader::Material::default();
	let descriptor = material.0;

	let descriptor_info = test_shader::Descriptor0::get();
	println!("Descriptor info {:?}", descriptor_info);

	println!("Hello, world!");
}

pub extern "C" fn on_load() {
	println!("On load successfully called!");
}

pub extern "C" fn on_unload() {
	println!("On unload successfully called!");
}

#[no_mangle]
pub extern "C" fn _goldfish_create_game_lib() -> GameLib {
	GameLib { on_load, on_unload }
}
