use super::{renderer::Vertex, GoldfishError, GoldfishResult};
use serde::{Deserialize, Serialize};

use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub enum AssetType
{
	Mesh,
	Texture,
	Shader,
	Other,
}

impl AssetType
{
	pub fn from_extension(extension: &str) -> Self
	{
		match extension.to_ascii_lowercase().as_str()
		{
			"png" | "jpg" | "jpeg" => Self::Texture,
			"fbx" | "obj" => Self::Mesh,
			"hlsl" => Self::Shader,
			_ => Self::Other,
		}
	}
}

pub enum Package
{
	// Mesh(MeshPackage),
	Shader(ShaderPackage),
	Text(String),
	Bin(Vec<u8>),
}

#[derive(Serialize, Deserialize)]
pub struct ShaderPackage
{
	pub vs_ir: Option<Vec<u32>>,
	pub ps_ir: Option<Vec<u32>>,
}

// #[derive(Serialize, Deserialize)]
// pub struct MeshPackage
// {
// 	pub vertices: Vec<Vertex>,
// }

pub type ReadAssetFn = fn(Uuid, AssetType) -> GoldfishResult<Package>;
