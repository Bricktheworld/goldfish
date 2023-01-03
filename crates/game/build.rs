#![allow(dead_code)]
#![allow(unused_imports)]

use hassle_rs::{Dxc, DxcIncludeHandler, HassleError};

use byteorder::{NativeEndian, WriteBytesExt};
use spirv_cross::{
	hlsl, spirv,
	spirv::{Decoration, Type},
};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
enum BuildError {
	#[error("A shader reflection error occurred {0}: {1}")]
	ShaderReflection(PathBuf, spirv_cross::ErrorCode),
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
				println!("cargo:warning=Compiling Failed to find included file {}", full_path.to_str().unwrap());
				None
			}
		}
	}
}

struct CompiledShaders {
	vs: Option<Vec<u32>>,
	ps: Option<Vec<u32>>,
	cs: Option<Vec<u32>>,
}

fn compile_hlsl(path: &Path, src: &str, disable_optimizations: bool) -> Result<(Vec<spirv::Ast<hlsl::Target>>, CompiledShaders), BuildError> {
	let dxc = Dxc::new(None).map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

	let compiler = dxc.create_compiler().map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;
	let library = dxc.create_library().map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

	let compile = |entry_point: &str, target_profile: &str, args: &[&str], defines: &[(&str, Option<&str>)]| -> Result<Vec<u32>, BuildError> {
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
				let error_blob = result.0.get_error_buffer().map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;
				Err(BuildError::ShaderCompilation(
					path.to_path_buf(),
					HassleError::CompileError(
						library
							.get_blob_as_string(&error_blob.into())
							.map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?,
					),
				))
			}
			Ok(result) => {
				let result_blob = result.get_result().map_err(move |err| BuildError::ShaderCompilation(path.to_path_buf(), err))?;

				Ok(result_blob.to_vec())
			}
		}
	};

	let mut asts: Vec<spirv::Ast<hlsl::Target>> = Default::default();

	let spirv_default = ["-spirv"];
	let spirv_no_optimize = ["-spirv", "-Od"];

	let config: &[&str] = if disable_optimizations { &spirv_no_optimize } else { &spirv_default };

	let vs = if src.contains(VS_MAIN) {
		let vs_ir = compile(VS_MAIN, "vs_6_0", config, &[])?;

		let module = spirv::Module::from_words(&vs_ir);
		let ast = spirv::Ast::<hlsl::Target>::parse(&module).map_err(move |err| BuildError::ShaderReflection(path.to_path_buf(), err))?;
		asts.push(ast);
		Some(vs_ir)
	} else {
		None
	};

	let ps = if src.contains(PS_MAIN) {
		let ps_ir = compile(PS_MAIN, "ps_6_0", config, &[])?;

		let module = spirv::Module::from_words(&ps_ir);
		let ast = spirv::Ast::<hlsl::Target>::parse(&module).map_err(move |err| BuildError::ShaderReflection(path.to_path_buf(), err))?;
		asts.push(ast);
		Some(ps_ir)
	} else {
		None
	};

	let cs = if src.contains(CS_MAIN) {
		let cs_ir = compile(CS_MAIN, "cs_6_0", config, &[])?;

		let module = spirv::Module::from_words(&cs_ir);
		let ast = spirv::Ast::<hlsl::Target>::parse(&module).map_err(move |err| BuildError::ShaderReflection(path.to_path_buf(), err))?;
		asts.push(ast);
		Some(cs_ir)
	} else {
		None
	};

	Ok((asts, CompiledShaders { vs, ps, cs }))
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum MemberType {
	F32,
	Vec2,
	Vec3,
	Vec4,
	Mat3,
	Mat4,
	U32,
	UVec2,
	UVec3,
	UVec4,
}
impl From<Type> for MemberType {
	fn from(ty: Type) -> Self {
		match ty {
			Type::Float { vecsize: 1, columns: 1, .. } => MemberType::F32,
			Type::Float { vecsize: 2, columns: 1, .. } => MemberType::Vec2,
			Type::Float { vecsize: 3, columns: 1, .. } => MemberType::Vec3,
			Type::Float { vecsize: 4, columns: 1, .. } => MemberType::Vec4,
			Type::Float { vecsize: 3, columns: 3, .. } => MemberType::Mat3,
			Type::Float { vecsize: 4, columns: 4, .. } => MemberType::Mat4,
			Type::UInt { vecsize: 1, columns: 1, .. } => MemberType::U32,
			Type::UInt { vecsize: 2, columns: 1, .. } => MemberType::UVec2,
			Type::UInt { vecsize: 3, columns: 1, .. } => MemberType::UVec3,
			Type::UInt { vecsize: 4, columns: 1, .. } => MemberType::UVec4,
			_ => unimplemented!("Unimplemented type {:?}", ty),
		}
	}
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct StructMember {
	name: String,
	ty: MemberType,
	offset: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct Struct {
	ty_name: String,
	members: Vec<StructMember>,
	size: u32,
}

#[derive(Debug)]
enum DescriptorBinding {
	CBuffer { name: String, struct_info: Struct },
	StructuredBuffer { name: String, struct_info: Struct },
	RWStructuredBuffer { name: String, struct_info: Struct },
	SamplerState { name: String },
	Texture2D { name: String },
}

type DescriptorBindings = HashMap<u32, DescriptorBinding>;
type DescriptorSets = HashMap<u32, DescriptorBindings>;

fn generate_descriptors(asts: &mut [spirv::Ast<hlsl::Target>]) -> DescriptorSets {
	let mut descriptors: DescriptorSets = Default::default();
	for ast in asts {
		let resources = ast.get_shader_resources().unwrap();
		for resource in resources.uniform_buffers {
			let ty_name = ast.get_name(resource.base_type_id).unwrap();
			let name = ast.get_name(resource.id).unwrap();

			let ty_name = if let Some(last) = ty_name.rfind(".") { ty_name[last + 1..].to_owned() } else { ty_name };

			let resource_type = ast.get_type(resource.base_type_id).unwrap();
			let size = ast.get_declared_struct_size(resource.base_type_id).unwrap();

			let Type::Struct { member_types, .. } = resource_type else {
                unimplemented!(
                    "Uniform buffers must be a struct! {:?}",
                    resource_type
                );
            };

			let members = member_types
				.iter()
				.enumerate()
				.map(|(i, id)| StructMember {
					name: ast.get_member_name(resource.base_type_id, i as u32).unwrap(),
					ty: ast.get_type(*id).unwrap().into(),
					offset: ast.get_member_decoration(resource.base_type_id, i as u32, Decoration::Offset).unwrap(),
				})
				.collect::<Vec<_>>();

			let set = ast.get_decoration(resource.id, Decoration::DescriptorSet).unwrap();

			let binding = ast.get_decoration(resource.id, Decoration::Binding).unwrap();

			descriptors.entry(set).or_default().entry(binding).or_insert(DescriptorBinding::CBuffer {
				name,
				struct_info: Struct { ty_name, members, size },
			});
		}

		for resource in resources.storage_buffers {
			println!("cargo:warning=Base type id {:?}", resource.base_type_id);
			let ty_name = ast.get_name(resource.base_type_id + 1).unwrap();
			let name = ast.get_name(resource.id).unwrap();

			// let ty_name = if let Some(last) = ty_name.rfind(".") { ty_name[last + 1..].to_owned() } else { ty_name };

			let resource_type = ast.get_type(resource.base_type_id).unwrap();

			let Type::Struct { ref member_types, .. } = resource_type else {
                unimplemented!(
                    "Storage buffers must be a struct! {:?}",
                    resource_type
                );
            };

			let base_type_id = member_types[0];

			let Type::Struct {ref member_types, ..} = ast.get_type(base_type_id).unwrap() else {
                unimplemented!("Storage buffer expected to have nested struct for some reason :/");
            };

			let size = ast.get_decoration(base_type_id, Decoration::ArrayStride).unwrap();

			let members = member_types
				.iter()
				.enumerate()
				.map(|(i, id)| StructMember {
					// TODO(Brandon): This cannot POSSIBLY be correct, but for some reason it's working :/
					name: ast.get_member_name(resource.base_type_id + 1, i as u32).unwrap(),
					ty: ast.get_type(*id).unwrap().into(),
					offset: ast.get_member_decoration(resource.base_type_id + 1, i as u32, Decoration::Offset).unwrap(),
				})
				.collect::<Vec<_>>();

			let set = ast.get_decoration(resource.id, Decoration::DescriptorSet).unwrap();

			let binding = ast.get_decoration(resource.id, Decoration::Binding).unwrap();

			let writeable = ast.get_decoration(resource.id, Decoration::NonWritable).unwrap() == 0;

			descriptors.entry(set).or_default().entry(binding).or_insert(if writeable {
				DescriptorBinding::StructuredBuffer {
					name,
					struct_info: Struct { ty_name, members, size },
				}
			} else {
				DescriptorBinding::RWStructuredBuffer {
					name,
					struct_info: Struct { ty_name, members, size },
				}
			});
		}

		for resource in resources.separate_samplers {
			let name = resource.name;

			let set = ast.get_decoration(resource.id, Decoration::DescriptorSet).unwrap();

			let binding = ast.get_decoration(resource.id, Decoration::Binding).unwrap();

			descriptors.entry(set).or_default().entry(binding).or_insert(DescriptorBinding::SamplerState { name });
		}

		for resource in resources.separate_images {
			let name = resource.name;

			let set = ast.get_decoration(resource.id, Decoration::DescriptorSet).unwrap();

			let binding = ast.get_decoration(resource.id, Decoration::Binding).unwrap();

			descriptors.entry(set).or_default().entry(binding).or_insert(DescriptorBinding::Texture2D { name });
		}
	}
	return descriptors;
}

fn parse_shader_includes(asset_dir: &Path) -> Result<HashMap<String, DescriptorSets>, BuildError> {
	let mut descriptor_layouts: HashMap<String, DescriptorSets> = Default::default();

	for asset in fs::read_dir(asset_dir).map_err(move |err| BuildError::Filesystem(err))? {
		let asset = asset.map_err(move |err| BuildError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir() {
			unimplemented!("Cannot handle nested directories for shaders");
		} else if let Some(extension) = asset_path.extension() {
			if extension != SHADER_INC {
				continue;
			}

			println!("cargo:warning=Parsing shader include {} ...", asset_path.to_str().unwrap());

			let mut src = fs::read_to_string(&asset_path).map_err(move |err| BuildError::Filesystem(err))?;

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
				let (mut asts, _) = compile_hlsl(&asset_path, &src, true)?;
				let descriptors = generate_descriptors(&mut asts);

				descriptor_layouts.insert(asset_path.file_stem().unwrap().to_str().unwrap().to_owned(), descriptors);
			}
		}
	}
	Ok(descriptor_layouts)
}

fn generate_descriptor_rust(set: u32, bindings: &DescriptorBindings) -> String {
	format!(
		"
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Descriptor{0} {{
{1}
}}
",
		set,
		bindings
			.iter()
			.map(|(_, info)| format!(
				"pub {}: {},\n",
				match info {
					DescriptorBinding::CBuffer { name, .. } => name,
					DescriptorBinding::StructuredBuffer { name, .. } => name,
					DescriptorBinding::RWStructuredBuffer { name, .. } => name,
					DescriptorBinding::SamplerState { name } => name,
					DescriptorBinding::Texture2D { name } => name,
				},
				match info {
					DescriptorBinding::CBuffer {
						struct_info: Struct { ty_name, .. }, ..
					} => ty_name,
					DescriptorBinding::StructuredBuffer {
						struct_info: Struct { ty_name, .. }, ..
					} => ty_name,
					DescriptorBinding::RWStructuredBuffer {
						struct_info: Struct { ty_name, .. }, ..
					} => ty_name,
					_ => "u8",
				},
			))
			.collect::<String>(),
	)
}
fn generate_struct_rust(struct_info: &Struct) -> String {
	format!(
		"
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct {0} {{
{1}
}}

unsafe impl bytemuck::Pod for {0} {{}}
unsafe impl bytemuck::Zeroable for {0} {{}}

",
		struct_info.ty_name,
		struct_info
			.members
			.iter()
			.map(|member| format!(
				"pub {}: {},\n",
				member.name,
				match member.ty {
					MemberType::F32 => "f32",
					MemberType::Vec2 => "glam::Vec2",
					MemberType::Vec3 => "glam::Vec3",
					MemberType::Vec4 => "glam::Vec4",
					MemberType::Mat3 => "glam::Mat3",
					MemberType::Mat4 => "glam::Mat4",
					MemberType::U32 => "u32",
					MemberType::UVec2 => "glam::UVec2",
					MemberType::UVec3 => "glam::UVec3",
					MemberType::UVec4 => "glam::UVec4",
				}
			))
			.collect::<String>(),
	)
}

fn generate_cbuffer_rust(struct_info: &Struct) -> String {
	format!(
		"
{1}

impl goldfish::build::CBuffer<{2}> for {0} {{
    fn size() -> usize {{
        {2}
    }}

	fn as_buffer(&self) -> [u8; {2}] {{
        let mut output: [u8; {2}] = [0; {2}];
        {3}
        output
    }}
}}
",
		struct_info.ty_name,
		generate_struct_rust(struct_info),
		struct_info.size,
		struct_info
			.members
			.iter()
			.map(|member| format!(
				"
let slice = {0};
output[{1}..{1} + slice.len()].clone_from_slice(slice);
",
				match member.ty {
					MemberType::F32 | MemberType::U32 => format!("&self.{0}.to_ne_bytes()", member.name),
					MemberType::Vec2 | MemberType::Vec3 | MemberType::Vec4 | MemberType::Mat3 | MemberType::Mat4 | MemberType::UVec2 | MemberType::UVec3 | MemberType::UVec4 =>
						format!("bytemuck::cast_slice::<_, u8>(self.{0}.as_ref())", member.name),
				},
				member.offset,
			))
			.collect::<String>(),
	)
}

fn generate_structured_buffer_rust(struct_info: &Struct) -> String {
	format!(
		"
{1}

impl goldfish::build::StructuredBuffer<{2}> for {0} {{
    fn size() -> usize {{
        {2}
    }}

	fn copy_to_raw(src: &[Self], dst: &mut [u8]) {{
        assert!(dst.len() >= Self::size() * src.len());
        for (i, buf) in src.iter().enumerate()
        {{
            let dst = &mut dst[i * Self::size()..];
            {3}
        }}
    }}
}}
",
		struct_info.ty_name,
		generate_struct_rust(struct_info),
		struct_info.size,
		struct_info
			.members
			.iter()
			.map(|member| format!(
				"
let slice = {0};
dst[{1}..{1} + slice.len()].clone_from_slice(slice);
",
				match member.ty {
					MemberType::F32 | MemberType::U32 => format!("&buf.{0}.to_ne_bytes()", member.name),
					MemberType::Vec2 | MemberType::Vec3 | MemberType::Vec4 | MemberType::Mat3 | MemberType::Mat4 | MemberType::UVec2 | MemberType::UVec3 | MemberType::UVec4 =>
						format!("bytemuck::cast_slice::<_, u8>(buf.{0}.as_ref())", member.name),
				},
				member.offset,
			))
			.collect::<String>(),
	)
}

fn compile_shaders(out_dir: &Path, asset_dir: &Path, descriptor_layouts: &HashMap<String, DescriptorSets>) -> Result<String, BuildError> {
	let mut generated = String::default();
	for asset in fs::read_dir(asset_dir).map_err(move |err| BuildError::Filesystem(err))? {
		let asset = asset.map_err(move |err| BuildError::Filesystem(err))?;
		let asset_path = asset.path();

		if asset_path.is_dir() {
			unimplemented!("Cannot handle nested directories for shaders");
		} else if let Some(extension) = asset_path.extension() {
			if extension != SHADER_EXT {
				continue;
			}

			println!("cargo:warning=Compiling {} ...", asset_path.to_str().unwrap());

			let src = fs::read_to_string(&asset_path).map_err(move |err| BuildError::Filesystem(err))?;

			let (mut asts, compiled_shaders) = compile_hlsl(&asset_path, &src, false)?;

			let mut shader_ir_consts = String::default();
			if let Some(ref vs) = compiled_shaders.vs {
				let bytes = vs.iter().flat_map(|code| code.to_ne_bytes()).collect::<Vec<_>>();

				let out = out_dir.join(asset_path.file_name().unwrap()).with_extension("vs");

				std::fs::write(&out, bytes).map_err(move |err| BuildError::Filesystem(err))?;

				shader_ir_consts += &format!(
					"pub const VS_BYTES: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{}\"));\n",
					out.file_name().unwrap().to_str().unwrap()
				);
			}

			if let Some(ref ps) = compiled_shaders.ps {
				let bytes = ps.iter().flat_map(|code| code.to_ne_bytes()).collect::<Vec<_>>();

				let out = out_dir.join(asset_path.file_name().unwrap()).with_extension("ps");
				std::fs::write(&out, bytes).map_err(move |err| BuildError::Filesystem(err))?;

				shader_ir_consts += &format!(
					"pub const PS_BYTES: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{}\"));\n",
					out.file_name().unwrap().to_str().unwrap()
				);
			}

			if let Some(ref cs) = compiled_shaders.cs {
				let bytes = cs.iter().flat_map(|code| code.to_ne_bytes()).collect::<Vec<_>>();

				let out = out_dir.join(asset_path.file_name().unwrap()).with_extension("cs");
				std::fs::write(&out, bytes).map_err(move |err| BuildError::Filesystem(err))?;

				shader_ir_consts += &format!(
					"pub const CS_BYTES: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{}\"));\n",
					out.file_name().unwrap().to_str().unwrap()
				);
			}

			let descriptors = generate_descriptors(&mut asts);

			let included_sets = descriptor_layouts
				.iter()
				.flat_map(|(include, sets)| {
					if src.contains(&format!("#include \"{}.hlsli\"", include)) {
						sets.iter()
							.map(|(set, _)| (*set, format!("super::{}_inc::Descriptor{}", include, *set)))
							.collect::<Vec<(u32, String)>>()
					} else {
						Default::default()
					}
				})
				.collect::<HashMap<u32, String>>();

			let mut descriptor_decls: Vec<String> = Default::default();
			let mut cbuffer_decls: Vec<Struct> = Default::default();
			let mut structured_buffer_decls: Vec<Struct> = Default::default();

			for (set, bindings) in descriptors {
				if let Some(descriptor_type) = included_sets.get(&set) {
					descriptor_decls.push(format!("\npub type Descriptor{} = {};\n", set, descriptor_type));
				} else {
					cbuffer_decls.append(
						&mut bindings
							.iter()
							.flat_map(|(_, info)| match info {
								DescriptorBinding::CBuffer { struct_info, .. } => Some(struct_info.clone()),
								_ => None,
							})
							.collect(),
					);
					structured_buffer_decls.append(
						&mut bindings
							.iter()
							.flat_map(|(_, info)| match info {
								DescriptorBinding::RWStructuredBuffer { struct_info, .. } => Some(struct_info.clone()),
								DescriptorBinding::StructuredBuffer { struct_info, .. } => Some(struct_info.clone()),
								_ => None,
							})
							.collect(),
					);
					descriptor_decls.push(generate_descriptor_rust(set, &bindings));
				}
			}

			use itertools::Itertools;
			let cbuffer_decls = cbuffer_decls.into_iter().unique().collect::<Vec<_>>();

			generated += &format!(
				"
pub mod {} {{
{}
{}

{}
{}
}}
",
				asset_path.file_stem().unwrap().to_str().unwrap(),
				&shader_ir_consts,
				descriptor_decls.join(""),
				cbuffer_decls.iter().map(|struct_info| generate_cbuffer_rust(struct_info)).collect::<String>(),
				structured_buffer_decls.iter().map(|struct_info| generate_structured_buffer_rust(struct_info)).collect::<String>(),
			);
		}
	}
	Ok(generated)
}

fn main() {
	let out_dir = &env::var_os("OUT_DIR").unwrap();
	println!("cargo:warning=Running build script, output dir {}", out_dir.to_str().unwrap());

	match parse_shader_includes(&Path::new(SHADERS_DIR)) {
		Err(err) => panic!("Failed to parse shader includes! {}", err),
		Ok(descriptor_layouts) => {
			let cbuffer_decls = descriptor_layouts
				.iter()
				.flat_map(|(_, sets)| {
					sets.iter().flat_map(|(_, bindings)| {
						bindings.iter().map(|(_, info)| match info {
							DescriptorBinding::CBuffer { struct_info, .. } => Some(struct_info),
							_ => None,
						})
					})
				})
				.flatten()
				.collect::<Vec<&Struct>>();

			let structured_buffer_decls = descriptor_layouts
				.iter()
				.flat_map(|(_, sets)| {
					sets.iter().flat_map(|(_, bindings)| {
						bindings.iter().map(|(_, info)| match info {
							DescriptorBinding::RWStructuredBuffer { struct_info, .. } => Some(struct_info),
							DescriptorBinding::StructuredBuffer { struct_info, .. } => Some(struct_info),
							_ => None,
						})
					})
				})
				.flatten()
				.collect::<Vec<&Struct>>();

			let includes_generated = descriptor_layouts
				.iter()
				.map(|(module, sets)| {
					format!(
						"
pub mod {}_inc {{
{}
{}
{}
}}",
						module,
						sets.iter().map(|(set, bindings)| generate_descriptor_rust(*set, bindings)).collect::<String>(),
						cbuffer_decls.iter().map(|struct_info| generate_cbuffer_rust(struct_info)).collect::<String>(),
						structured_buffer_decls.iter().map(|struct_info| generate_structured_buffer_rust(struct_info)).collect::<String>(),
					)
				})
				.collect::<String>();

			match compile_shaders(Path::new(&out_dir), Path::new(SHADERS_DIR), &descriptor_layouts) {
				Err(err) => panic!("Failed to compile shaders! {}", err),
				Ok(generated) => {
					println!("cargo:warning=Successfully compiled shaders!");

					let dst_path = Path::new(&out_dir).join("materials.rs");
					std::fs::write(&dst_path, &(includes_generated + &generated)).expect("Failed to write generated materials!");
				}
			}
		}
	}
}
