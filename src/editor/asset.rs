use serde::{Deserialize, Serialize};
// use std::marker::PhantomData;
use goldfish::renderer::TextureFormat;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const ASSET_META_EXTENSION: &'static str = ".meta";

#[derive(Serialize, Deserialize)]
pub enum Asset
{
	Mesh(Uuid),
	Texture(Uuid, TextureAsset),
}

#[derive(Serialize, Deserialize)]
pub struct TextureAsset
{
	format: TextureFormat,
}

pub fn import_assets(asset_dir: &Path) -> Vec<PathBuf>
{
	let mut paths: Vec<PathBuf> = vec![];

	for asset in fs::read_dir(asset_dir).expect("Failed to read resource directory")
	{
		let asset = asset.expect("Failed to get resource entry!");

		if asset.path().is_dir()
		{
			paths.append(&mut import_assets(asset.path().as_path()));
		}
		else
		{
			paths.push(asset.path());
			// println!(
			// 	"Resource entry {}",
			// 	resource.path().as_path().to_str().unwrap()
			// );
		}
	}

	return paths;
}

// pub trait LoadableResource
// {
// 	fn load() -> Self;
// }

// pub struct Resource<T: LoadableResource>
// {
// 	phantom: PhantomData<T>,
// }

// impl<T: LoadableResource> Resource<T>
// {
// 	pub fn new(uuid: Uuid) -> Self
// 	{
// 		todo!()
// 	}

// 	pub fn get(&self) -> Option<&T>
// 	{
// 		None
// 	}
// }
