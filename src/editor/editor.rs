mod asset;
use goldfish::GoldfishEngine;
use std::path::Path;

const ASSET_DIR: &'static str = "assets/";
const BUILD_DIR: &'static str = ".build/";
const BUILD_ASSET_DIR: &'static str = ".build/assets/";

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

	let assets = asset::import_assets(Path::new(ASSET_DIR));
	for asset in assets
	{}

	let engine = GoldfishEngine::new("Goldfish Editor");

	engine.run(move |_, _| {
		// println!("Editor update!");
	});
}
