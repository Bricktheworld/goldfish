#![allow(dead_code)]
#![allow(unused_imports)]

mod asset;
mod mesh_importer;
mod shader_compiler;
use goldfish::game::{CreateGamelibApi, GameLib};
use goldfish::GoldfishEngine;
use libloading::{Library, Symbol};
use std::path::Path;
use thiserror::Error;

use asset::read_asset;

const ASSET_DIR: &'static str = "assets/";
const BUILD_DIR: &'static str = ".build/";
const BUILD_ASSET_DIR: &'static str = ".build/assets/";

#[derive(Error, Debug)]
pub enum EditorError {
	#[error("Failed to import mesh: {0}")]
	MeshImport(russimp::RussimpError),
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

fn main() {
	if !Path::new(BUILD_DIR).is_dir() {
		panic!("Failed to find build directory!");
	}

	if !Path::new(ASSET_DIR).is_dir() {
		panic!("Failed to find resource directory!");
	}

	let lib = unsafe {
		Library::new(Path::new("target/debug/libgame.so")).expect("Failed to load libgame!")
	};

	let game_lib = unsafe {
		lib.get::<Symbol<CreateGamelibApi>>(b"_goldfish_create_game_lib")
			.expect("No gamelib constructor found!")()
	};

	match asset::import_assets(Path::new(ASSET_DIR)) {
		Err(err) => panic!("Failed to import assets: {}", err),
		_ => (),
	}

	let mut engine = GoldfishEngine::new("Goldfish Editor", read_asset);

	(game_lib.on_load)(&mut engine);

	engine.run(|engine, _| {
		(game_lib.on_update)(engine);
	});

	(game_lib.on_unload)(&mut engine);
}
