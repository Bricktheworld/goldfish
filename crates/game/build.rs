#![allow(dead_code)]
#![allow(unused_imports)]

use hassle_rs::{Dxc, DxcIncludeHandler, HassleError};

use byteorder::{NativeEndian, WriteBytesExt};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
enum BuildError {
	#[error("A shader compilation error occurred compiling {0}: {1}")]
	ShaderCompilation(PathBuf, HassleError),
	#[error("An unknown filesystem error occurred: {0}")]
	Filesystem(std::io::Error),
	#[error("Unknown error: {0}")]
	Unknown(String),
}

const SHADERS_DIR: &'static str = "shaders/";
const SHADER_EXT: &'static str = "hlsl";

const VS_MAIN: &'static str = "vs_main";
const PS_MAIN: &'static str = "ps_main";
const CS_MAIN: &'static str = "cs_main";

struct ShaderIncludeHandler<'a> {
	path: &'a Path,
}

impl<'a> DxcIncludeHandler for ShaderIncludeHandler<'a> {
	fn load_source(&mut self, filename: String) -> Option<String> {
		let full_path = self.path.join(filename);

		use std::io::Read;
		match std::fs::File::open(&full_path) {
			Ok(mut f) => {
				let mut content = String::new();
				f.read_to_string(&mut content).ok()?;
				Some(content)
			}
			Err(_) => {
				println!(
					"Failed to find included file {}",
					full_path.to_str().unwrap()
				);
				None
			}
		}
	}
}

fn generate_material(name: &str, bytes: &[u8]) -> String {
	let module = spirv_reflect::ShaderModule::load_u8_data(bytes).unwrap();
	let descriptor_sets = module.enumerate_descriptor_sets(None).unwrap();

	let mut declarations: Vec<String> = Default::default();

	let get_member_type =
		|type_description: &spirv_reflect::types::ReflectTypeDescription| -> &'static str {
			let member_type = type_description.type_flags;
			if member_type == spirv_reflect::types::ReflectTypeFlags::FLOAT {
				"f32"
			} else if member_type == spirv_reflect::types::ReflectTypeFlags::INT {
				"s32"
			} else if member_type
				== spirv_reflect::types::ReflectTypeFlags::VECTOR
					| spirv_reflect::types::ReflectTypeFlags::FLOAT
			{
				match type_description.traits.numeric.vector.component_count {
					2 => "Vec2",
					3 => "Vec3",
					4 => "Vec4",
					_ => unimplemented!(),
				}
			} else if member_type
				== spirv_reflect::types::ReflectTypeFlags::MATRIX
					| spirv_reflect::types::ReflectTypeFlags::VECTOR
					| spirv_reflect::types::ReflectTypeFlags::FLOAT
			{
				if type_description.traits.numeric.matrix.column_count
					!= type_description.traits.numeric.matrix.row_count
				{
					unimplemented!()
				} else {
					match type_description.traits.numeric.matrix.row_count {
						2 => "Mat2",
						3 => "Mat3",
						4 => "Mat4",
						_ => unimplemented!(),
					}
				}
			} else {
				unimplemented!("{:?}", member_type);
			}
		};

	let gen_uniform_struct =
		|type_desc: &spirv_reflect::types::ReflectTypeDescription| -> (String, String) {
			let index = type_desc.type_name.rfind(".").unwrap() + 1;

			let uniform_name = format!("{}", &type_desc.type_name[index..]);
			let declaration = format!(
				"\npub struct {} \n{{ \n {}}}",
				uniform_name,
				type_desc
					.members
					.iter()
					.map(|member| -> String {
						let member_type = get_member_type(member);

						format!("\t{}: {},\n", &member.struct_member_name, member_type)
					})
					.collect::<String>()
			);

			(declaration, uniform_name)
		};

	let gen_bindings = |bindings: &[spirv_reflect::types::ReflectDescriptorBinding]| {
		let mut declarations: Vec<String> = Default::default();

		(
			bindings
				.iter()
				.map(|binding| -> String {
					format!(
						"\t{}: {},\n",
						&binding.name,
						&match binding.descriptor_type {
							spirv_reflect::types::ReflectDescriptorType::Sampler =>
								"Sampler".to_owned(),
							spirv_reflect::types::ReflectDescriptorType::SampledImage =>
								"SampledImage".to_owned(),
							spirv_reflect::types::ReflectDescriptorType::UniformBuffer => {
								let (declaration, uniform_name) =
									gen_uniform_struct(&binding.type_description.as_ref().unwrap());

								declarations.push(declaration);
								uniform_name
							}
							_ => "Unimplemented".to_owned(),
						}
					)
				})
				.collect::<String>(),
			declarations,
		)
	};

	let material_declaration = format!(
		"\n pub struct {} ({});",
		name,
		descriptor_sets
			.iter()
			.map(|set| -> String {
				let set_name = format!("{}Descriptor{}", name, &set.set.to_string());
				let (generated, mut generated_declarations) = gen_bindings(&set.bindings);
				let declaration = format!("\npub struct {}\n{{ \n{} }}", &set_name, &generated);

				declarations.append(&mut generated_declarations);
				declarations.push(declaration);
				set_name
			})
			.collect::<String>()
	);

	declarations.join("") + &material_declaration
}

fn compile_hlsl(path: &Path, src: &str) -> Result<String, BuildError> {
	let dxc = Dxc::new(None)
		.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

	let compiler = dxc
		.create_compiler()
		.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;
	let library = dxc
		.create_library()
		.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

	let compile = |entry_point: &str,
	               target_profile: &str,
	               args: &[&str],
	               defines: &[(&str, Option<&str>)]|
	 -> Result<Vec<u32>, BuildError> {
		let blob = library
			.create_blob_with_encoding_from_str(src)
			.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

		let result = compiler.compile(
			&blob,
			path.file_name().unwrap().to_str().unwrap(),
			entry_point,
			target_profile,
			args,
			Some(&mut ShaderIncludeHandler {
				path: path.parent().unwrap_or(Path::new("./")),
			}),
			defines,
		);

		match result {
			Err(result) => {
				let error_blob = result
					.0
					.get_error_buffer()
					.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;
				Err(BuildError::ShaderCompilation(
					path.to_path_buf(),
					HassleError::CompileError(
						library
							.get_blob_as_string(&error_blob.into())
							.map_err(move |err| {
								BuildError::ShaderCompilation(path.to_path_buf(), err)
							})?,
					),
				))
			}
			Ok(result) => {
				let result_blob = result
					.get_result()
					.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

				Ok(result_blob.to_vec())
			}
		}
	};

	if src.contains(VS_MAIN) {
		let vs_ir = compile(VS_MAIN, "vs_6_0", &["-spirv"], &[])?;

		let bytes = vs_ir
			.iter()
			.flat_map(|code| code.to_ne_bytes())
			.collect::<Vec<_>>();

		return Ok(generate_material("TestMaterial", &bytes));
	}

	Ok(String::default())
}

fn compile_shaders(asset_dir: &Path) -> Result<String, BuildError> {
	let mut generated = String::default();
	for asset in fs::read_dir(asset_dir).map_err(move |err| BuildError::Filesystem(err))? {
		let asset = asset.map_err(move |err| BuildError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir() {
			compile_shaders(&asset_path)?;
		} else if let Some(extension) = asset_path.extension() {
			if extension != SHADER_EXT {
				continue;
			}

			println!(
				"cargo:warning=Compiling {} ...",
				asset_path.to_str().unwrap()
			);
			let src =
				fs::read_to_string(&asset_path).map_err(move |err| BuildError::Filesystem(err))?;

			generated += &compile_hlsl(&asset_path, &src)?;
		}
	}
	Ok(generated)
}

fn main() {
	let out_dir = &env::var_os("OUT_DIR").unwrap();

	match compile_shaders(&Path::new(SHADERS_DIR)) {
		Err(err) => panic!("Failed to compile shaders! {}", err),
		Ok(generated) => {
			println!("cargo:warning=Successfully compiled shaders!");
			let dst_path = Path::new(&out_dir).join("materials.rs");
			std::fs::write(&dst_path, &generated).expect("Failed to write generated materials!");
		}
	}
}
