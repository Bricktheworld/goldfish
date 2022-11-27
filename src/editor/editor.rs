mod asset;
mod shader_compiler;
use goldfish::GoldfishEngine;
use std::path::Path;
use thiserror::Error;

use asset::read_asset;

const ASSET_DIR: &'static str = "assets/";
const BUILD_DIR: &'static str = ".build/";
const BUILD_ASSET_DIR: &'static str = ".build/assets/";

#[derive(Error, Debug)]
pub enum EditorError
{
	#[error("Failed to compile shader: {0}")]
	ShaderCompilation(hassle_rs::HassleError),
	#[error("Failed to reflect spirv: {0}")]
	ShaderReflection(rspirv_reflect::ReflectError),
	#[error("Failed to serialize")]
	Serialize,
	#[error("Failed to deserialize")]
	Deserialize,
	#[error("An unknown OS filesystem error occurred")]
	Filesystem(std::io::Error),
	#[error("An unknown error occurred")]
	Unknown,
}

fn main()
{
	if !Path::new(BUILD_DIR).is_dir()
	{
		panic!("Failed to find build directory!");
	}

	if !Path::new(ASSET_DIR).is_dir()
	{
		panic!("Failed to find resource directory!");
	}

	match asset::import_assets(Path::new(ASSET_DIR))
	{
		Err(err) => panic!("Failed to import assets: {}", err),
		_ => (),
	}

	let engine = GoldfishEngine::new("Goldfish Editor", read_asset);

	engine.run(move |_, _| {
		// println!("Editor update!");
	});
}
