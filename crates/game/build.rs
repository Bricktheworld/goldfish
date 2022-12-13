#![allow(dead_code)]
#![allow(unused_imports)]

use hassle_rs::{Dxc, DxcIncludeHandler, HassleError};

use byteorder::{NativeEndian, WriteBytesExt};
use std::collections::HashMap;
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
const SHADER_INC: &'static str = "hlsli";

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

fn generate_material(
	src: &str,
	descriptor_sets: &[spirv_reflect::types::ReflectDescriptorSet],
	descriptor_map: Option<&HashMap<String, Vec<u32>>>,
) -> String {
	let mut descriptor_decls: Vec<String> = Default::default();
	let mut descriptor_impls: Vec<String> = Default::default();
	let mut uniform_decls: Vec<String> = Default::default();
	let mut uniform_impls: Vec<String> = Default::default();

	let included_descriptors = if let Some(descriptor_map) = descriptor_map {
		descriptor_map
			.iter()
			.flat_map(|(inc, descriptors)| -> Vec<(u32, String)> {
				if src.contains(&format!("#include \"{}.hlsli\"", inc)) {
					descriptors
						.iter()
						.map(|set| (*set, format!("super::{}_inc::Descriptor{}", inc, set)))
						.collect::<_>()
				} else {
					Default::default()
				}
			})
			.collect::<HashMap<_, _>>()
	} else {
		Default::default()
	};

	let get_member_type =
		|type_description: &spirv_reflect::types::ReflectTypeDescription| -> &'static str {
			let member_type = type_description.type_flags;
			if member_type == spirv_reflect::types::ReflectTypeFlags::FLOAT {
				"f32"
			} else if member_type
				== spirv_reflect::types::ReflectTypeFlags::VECTOR
					| spirv_reflect::types::ReflectTypeFlags::FLOAT
			{
				match type_description.traits.numeric.vector.component_count {
					2 => "glam::Vec2",
					3 => "glam::Vec3",
					4 => "glam::Vec4",
					_ => unimplemented!(
						"{:?} component_count: {}",
						member_type,
						type_description.traits.numeric.vector.component_count
					),
				}
			} else if member_type
				== spirv_reflect::types::ReflectTypeFlags::MATRIX
					| spirv_reflect::types::ReflectTypeFlags::VECTOR
					| spirv_reflect::types::ReflectTypeFlags::FLOAT
			{
				if type_description.traits.numeric.matrix.column_count
					!= type_description.traits.numeric.matrix.row_count
				{
					unimplemented!(
						"Non-matching matrix row and col count {} {}",
						type_description.traits.numeric.matrix.column_count,
						type_description.traits.numeric.matrix.row_count
					)
				} else {
					match type_description.traits.numeric.matrix.row_count {
						2 => "glam::Mat2",
						3 => "glam::Mat3",
						4 => "glam::Mat4",
						_ => unimplemented!(
							"{:?} row_count: {}",
							member_type,
							type_description.traits.numeric.matrix.row_count
						),
					}
				}
			} else {
				unimplemented!("{:?}", member_type);
			}
		};

	let mut gen_uniform_struct =
		|type_desc: &spirv_reflect::types::ReflectTypeDescription| -> String {
			let index = type_desc.type_name.rfind(".").unwrap() + 1;

			let uniform_name = format!("{}", &type_desc.type_name[index..]);
			let u_decl = format!(
				"\n#[derive(Clone, Copy, PartialEq, Default)]\npub struct {} {{\n{}}}",
				uniform_name,
				type_desc
					.members
					.iter()
					.map(|member| -> String {
						let member_type = get_member_type(member);

						format!("\tpub {}: {},\n", &member.struct_member_name, member_type)
					})
					.collect::<String>()
			);

			let u_impl = format!(
				"\nimpl goldfish::build::UniformBuffer for {} {{\n}}",
				uniform_name
			);

			uniform_decls.push(u_decl);
			uniform_impls.push(u_impl);
			uniform_name
		};

	let mut gen_bindings = |bindings: &[spirv_reflect::types::ReflectDescriptorBinding]| -> String {
		bindings
			.iter()
			.map(|binding| -> String {
				format!(
					"\tpub {}: {},\n",
					&binding.name,
					&match binding.descriptor_type {
						spirv_reflect::types::ReflectDescriptorType::Sampler =>
							"Sampler".to_owned(),
						spirv_reflect::types::ReflectDescriptorType::SampledImage =>
							"SampledImage".to_owned(),
						spirv_reflect::types::ReflectDescriptorType::UniformBuffer => {
							let uniform_name =
								gen_uniform_struct(&binding.type_description.as_ref().unwrap());

							uniform_name
						}
						_ => unimplemented!("{:?}", binding.descriptor_type),
					}
				)
			})
			.collect::<String>()
	};

	let gen_descriptor_info = |set: &spirv_reflect::types::ReflectDescriptorSet| -> String {
		set.bindings
			.iter()
			.map(|binding| {
				format!(
					"({}, {}),\n",
					binding.binding,
					match binding.descriptor_type {
						spirv_reflect::types::ReflectDescriptorType::Sampler =>
							"goldfish::renderer::DescriptorBindingType::Sampler",
						spirv_reflect::types::ReflectDescriptorType::SampledImage =>
							"goldfish::renderer::DescriptorBindingType::SampledImage",
						spirv_reflect::types::ReflectDescriptorType::UniformBuffer =>
							"goldfish::renderer::DescriptorBindingType::UniformBuffer",
						_ => unimplemented!("{:?}", binding.descriptor_type),
					}
				)
			})
			.collect::<_>()
	};

	let mut gen_descriptor = |set: &spirv_reflect::types::ReflectDescriptorSet| -> String {
		let set_name = format!("Descriptor{}", &set.set.to_string());

		if let Some(descriptor_type) = included_descriptors.get(&set.set) {
			descriptor_decls.push(format!(
				"pub type Descriptor{} = {};",
				set.set, descriptor_type
			));
		} else {
			let generated = gen_bindings(&set.bindings);
			let declaration = format!(
				"\n#[derive(Clone, Copy, PartialEq, Default)]\npub struct {} {{\n{}}}",
				&set_name, &generated
			);

			let implementation = format!(
				"
impl goldfish::build::DescriptorInfo for {} {{
    fn get() -> goldfish::renderer::DescriptorSetInfo {{
        goldfish::renderer::DescriptorSetInfo {{
            bindings: std::collections::HashMap::from([
                {}
            ]),
        }}
    }}
}}",
				set_name,
				gen_descriptor_info(&set),
			);
			descriptor_decls.push(declaration);
			descriptor_impls.push(implementation);
		}
		set_name
	};

	let set_count = descriptor_sets
		.iter()
		.map(|set| set.set + 1)
		.max()
		.unwrap_or(0u32);

	let material_declaration = format!(
		"\n#[derive(Clone, Copy, PartialEq, Default)]\npub struct Material({});",
		(0..set_count)
			.map(|set| -> String {
				let Some(set) = descriptor_sets.iter().find(|i| i.set == set) else {
					return "std::marker::PhantomData<()>".to_owned();
				};

				"pub ".to_owned() + &gen_descriptor(set)
			})
			.collect::<String>()
	);

	format!(
		"\n{}\n{}\n{}\n{}\n{}\n",
		uniform_decls.join(""),
		descriptor_decls.join(""),
		if descriptor_map.is_some() {
			&material_declaration
		} else {
			""
		},
		uniform_impls.join(""),
		descriptor_impls.join(""),
	)
}

fn compile_hlsl(
	path: &Path,
	src: &str,
	disable_optimizations: bool,
	descriptor_map: Option<&HashMap<String, Vec<u32>>>,
) -> Result<(String, Vec<u32>), BuildError> {
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

	let shader_module_name = path.file_stem().unwrap().to_str().unwrap().to_owned()
		+ if descriptor_map.is_some() { "" } else { "_inc" };
	let mut descriptor_sets: Vec<spirv_reflect::types::ReflectDescriptorSet> = Default::default();

	let spirv_default = ["-spirv"];
	let spirv_no_optimize = ["-spirv", "-Od"];

	let config: &[&str] = if disable_optimizations {
		&spirv_no_optimize
	} else {
		&spirv_default
	};

	if src.contains(VS_MAIN) {
		let vs_ir = compile(VS_MAIN, "vs_6_0", config, &[])?;

		let bytes = vs_ir
			.iter()
			.flat_map(|code| code.to_ne_bytes())
			.collect::<Vec<_>>();

		let module = spirv_reflect::ShaderModule::load_u8_data(&bytes).unwrap();
		descriptor_sets.append(&mut module.enumerate_descriptor_sets(None).unwrap());
	}

	if src.contains(PS_MAIN) {
		let ps_ir = compile(PS_MAIN, "ps_6_0", config, &[])?;

		let bytes = ps_ir
			.iter()
			.flat_map(|code| code.to_ne_bytes())
			.collect::<Vec<_>>();

		let module = spirv_reflect::ShaderModule::load_u8_data(&bytes).unwrap();
		descriptor_sets.append(&mut module.enumerate_descriptor_sets(None).unwrap());
	}
	descriptor_sets.dedup();

	let material = generate_material(src, &descriptor_sets, descriptor_map);

	Ok((
		format!("pub mod {} {{\n{}\n}}", &shader_module_name, material),
		descriptor_sets.iter().map(|set| set.set).collect::<_>(),
	))
}

fn parse_shader_includes(
	asset_dir: &Path,
) -> Result<(String, HashMap<String, Vec<u32>>), BuildError> {
	let mut generated = String::default();
	let mut descriptor_map: HashMap<String, Vec<u32>> = Default::default();
	for asset in fs::read_dir(asset_dir).map_err(move |err| BuildError::Filesystem(err))? {
		let asset = asset.map_err(move |err| BuildError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir() {
			// parse_shader_includes(&asset_path)?;
			unimplemented!("Cannot handle nested directories for shaders");
		} else if let Some(extension) = asset_path.extension() {
			if extension != SHADER_INC {
				continue;
			}

			println!(
				"cargo:warning=Parsing shader include {} ...",
				asset_path.to_str().unwrap()
			);

			let mut src =
				fs::read_to_string(&asset_path).map_err(move |err| BuildError::Filesystem(err))?;

			if src.contains("#include") {
				unimplemented!("Cannot have nested includes, as this would require a dependency tree which is not implemented...");
			}

			if !src.contains(VS_MAIN) {
				src += "
struct __VS_OUTPUT__
{
    float4 position : SV_POSITION;
};

__VS_OUTPUT__ vs_main(float3 pos : POSITION)
{
    __VS_OUTPUT__ result;
    result.position = float4(0.0, 0.0, 0.0, 0.0);
    return result;
}
";
				let (gen, descriptors) = compile_hlsl(&asset_path, &src, true, None)?;
				generated += &gen;

				descriptor_map.insert(
					asset_path.file_stem().unwrap().to_str().unwrap().to_owned(),
					descriptors,
				);
			}
		}
	}
	Ok((generated, descriptor_map))
}

fn compile_shaders(
	asset_dir: &Path,
	descriptor_map: &HashMap<String, Vec<u32>>,
) -> Result<String, BuildError> {
	let mut generated = String::default();
	for asset in fs::read_dir(asset_dir).map_err(move |err| BuildError::Filesystem(err))? {
		let asset = asset.map_err(move |err| BuildError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir() {
			// compile_shaders(&asset_path)?;
			unimplemented!("Cannot handle nested directories for shaders");
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

			generated += &compile_hlsl(&asset_path, &src, false, Some(descriptor_map))?.0;
		}
	}
	Ok(generated)
}

fn main() {
	let out_dir = &env::var_os("OUT_DIR").unwrap();

	match parse_shader_includes(&Path::new(SHADERS_DIR)) {
		Err(err) => panic!("Failed to parse shader includes! {}", err),
		Ok((includes_generated, descriptor_map)) => {
			println!("cargo:warning=Successfully parsed shader includes!");

			match compile_shaders(&Path::new(SHADERS_DIR), &descriptor_map) {
				Err(err) => panic!("Failed to compile shaders! {}", err),
				Ok(generated) => {
					println!("cargo:warning=Successfully compiled shaders!");
					let dst_path = Path::new(&out_dir).join("materials.rs");
					std::fs::write(&dst_path, includes_generated + &generated)
						.expect("Failed to write generated materials!");
				}
			}
		}
	}
}
