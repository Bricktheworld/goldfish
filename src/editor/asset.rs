use super::shader_compiler;
use super::{EditorError, BUILD_ASSET_DIR};
use bincode::serialize;
use filetime::FileTime;
use goldfish::package::{AssetType, Package, ShaderPackage};
use goldfish::renderer::TextureFormat;
use goldfish::{GoldfishError, GoldfishResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use uuid::Uuid;

const ASSET_META_EXTENSION: &'static str = "meta";
const BUILD_ASSET_EXTENSION: &'static str = "asset";

#[derive(Serialize, Deserialize, PartialEq, PartialOrd, Eq)]
pub struct Version
{
	version: u32,
}

impl Version
{
	pub const fn new(major: u16, minor: u16) -> Self
	{
		Self {
			version: ((major as u32) << 16) | minor as u32,
		}
	}
}

// TODO(Brandon): Test this out
impl Ord for Version
{
	fn cmp(&self, other: &Self) -> std::cmp::Ordering
	{
		let self_major = self.version >> 16;
		let other_major = other.version >> 16;

		if self_major < other_major
		{
			return std::cmp::Ordering::Less;
		}
		else if self_major > other_major
		{
			return std::cmp::Ordering::Greater;
		}
		else
		{
			let self_minor = self.version & 0xFFFF;
			let other_minor = other.version & 0xFFFF;

			if self_minor < other_minor
			{
				return std::cmp::Ordering::Less;
			}
			else if self_minor > other_minor
			{
				return std::cmp::Ordering::Greater;
			}
			else
			{
				return std::cmp::Ordering::Equal;
			}
		}
	}
}

#[derive(Serialize, Deserialize)]
pub enum AdditionalAssetData
{
	Mesh,
	Texture(TextureAsset),
	Shader,
	Other,
}

#[derive(Serialize, Deserialize)]
pub struct Asset
{
	pub uuid: Uuid,
	pub version: Version,
	pub asset_type: AssetType,
	pub additional_data: AdditionalAssetData,
}

#[derive(Serialize, Deserialize)]
pub struct TextureAsset
{
	pub format: TextureFormat,
}

impl Asset
{
	const CURRENT_ASSET_VERSION: Version = Version::new(1, 0);

	pub fn new(asset_type: AssetType) -> Self
	{
		let uuid = Uuid::new_v4();

		let additional_data = match asset_type
		{
			AssetType::Mesh => AdditionalAssetData::Mesh,
			AssetType::Texture => AdditionalAssetData::Texture(TextureAsset {
				format: TextureFormat::RGBA8,
			}),
			AssetType::Shader => AdditionalAssetData::Shader,
			AssetType::Other => AdditionalAssetData::Other,
		};

		Self {
			uuid,
			version: Self::CURRENT_ASSET_VERSION,
			asset_type,
			additional_data,
		}
	}
}

pub fn import_assets(asset_dir: &Path) -> Result<(), EditorError>
{
	if !Path::new(BUILD_ASSET_DIR).is_dir()
	{
		fs::create_dir(BUILD_ASSET_DIR).map_err(move |err| EditorError::Filesystem(err))?;
	}

	for asset in fs::read_dir(asset_dir).map_err(move |err| EditorError::Filesystem(err))?
	{
		let asset = asset.map_err(move |err| EditorError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir()
		{
			import_assets(asset_path.as_path())?;
		}
		else if asset_path.extension().unwrap_or_default() != ASSET_META_EXTENSION
		{
			let meta_extension = if let Some(extension) = asset_path.extension()
			{
				extension.to_str().unwrap().to_owned() + "." + ASSET_META_EXTENSION
			}
			else
			{
				ASSET_META_EXTENSION.to_owned()
			};

			let meta_path = asset_path.with_extension(&meta_extension);

			let asset_type = AssetType::from_extension(
				asset_path.extension().unwrap_or_default().to_str().unwrap(),
			);

			let mut meta_file_was_created = false;

			let asset = if meta_path.exists()
			{
				match fs::read_to_string(&meta_path)
				{
					Ok(contents) => match serde_json::from_str::<Asset>(contents.as_str())
					{
						Ok(asset) => asset,
						Err(err) =>
						{
							println!("Failed to deserialize metadata for asset! {}", err);
							continue;
						}
					},
					Err(err) =>
					{
						println!("Failed to load metadata for asset! {}", err);
						continue;
					}
				}
			}
			else
			{
				println!(
					"Failed to find meta file {}! Creating...",
					meta_path.as_path().to_str().unwrap()
				);

				let metadata = Asset::new(asset_type);

				let serialized = serde_json::to_string_pretty(&metadata)
					.map_err(move |_| EditorError::Serialize)?;

				fs::write(&meta_path, serialized)
					.map_err(move |err| EditorError::Filesystem(err))?;
				meta_file_was_created = true;

				metadata
			};

			let build_path = Path::new(BUILD_ASSET_DIR)
				.join(asset.uuid.to_string())
				.with_extension(BUILD_ASSET_EXTENSION);

			let mut needs_reimport = asset.version != Asset::CURRENT_ASSET_VERSION
				|| meta_file_was_created
				|| !build_path.is_file();

			if !needs_reimport
			{
				let build_meta =
					fs::metadata(&build_path).map_err(move |err| EditorError::Filesystem(err))?;
				let asset_meta =
					fs::metadata(&asset_path).map_err(move |err| EditorError::Filesystem(err))?;
				let meta_meta =
					fs::metadata(&meta_path).map_err(move |err| EditorError::Filesystem(err))?;

				let asset_modified_time = FileTime::from_last_modification_time(&asset_meta);
				let build_modified_time = FileTime::from_last_modification_time(&build_meta);
				let meta_modified_time = FileTime::from_last_modification_time(&meta_meta);

				needs_reimport = asset_modified_time > build_modified_time
					|| meta_modified_time > build_modified_time;
			}

			if needs_reimport
			{
				let serialized = match asset.asset_type
				{
					AssetType::Shader =>
					{
						let shader_data = fs::read_to_string(&asset_path)
							.map_err(move |err| EditorError::Filesystem(err))?;
						let shader_asset =
							shader_compiler::compile_hlsl(&asset_path, &shader_data)?;

						Some(
							bincode::serialize(&shader_asset)
								.map_err(move |_| EditorError::Serialize)?,
						)
					}
					_ => None,
				};

				if let Some(serialized) = serialized
				{
					let mut output = fs::File::create(&build_path)
						.map_err(move |err| EditorError::Filesystem(err))?;
					output
						.write_all(&serialized)
						.map_err(move |err| EditorError::Filesystem(err))?;

					// Touch asset files
					let now = FileTime::now();
					if let Err(err) = filetime::set_file_mtime(&build_path, now)
					{
						println!("WARNING: Failed to update date modified for build file {}! Maybe it wasn't created properly? {}", build_path.as_path().to_str().unwrap_or("UNKNOWN_BUILD_PATH"), err);
					}

					if let Err(err) = filetime::set_file_mtime(&meta_path, now)
					{
						println!("WARNING: Failed to update date modified for metadata file! Maybe it wasn't created properly? {}", err);
					}
				}
				else
				{
					println!("No output was created for asset {}!", asset.uuid);
				}
			}
		}
	}
	Ok(())
}

pub fn read_asset(uuid: Uuid, asset_type: AssetType) -> GoldfishResult<Package>
{
	let build_path = Path::new(BUILD_ASSET_DIR)
		.join(uuid.to_string())
		.with_extension(BUILD_ASSET_EXTENSION);

	match asset_type
	{
		AssetType::Shader =>
		{
			let contents =
				fs::read(&build_path).map_err(move |err| GoldfishError::Filesystem(err))?;

			let package = bincode::deserialize::<ShaderPackage>(&contents).map_err(move |err| {
				GoldfishError::Unknown(
					"Failed to deserialize shader package: ".to_string()
						+ &err.to_string() + ". Try cleaning '.build' and reimporting all assets.",
				)
			})?;

			Ok(Package::Shader(package))
		}
		_ => Err(GoldfishError::Unknown("Not handling yet!".to_string())),
	}
}
