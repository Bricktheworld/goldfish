use super::EditorError;
use goldfish::{
	package::ShaderPackage,
	renderer::{CS_MAIN, PS_MAIN, VS_MAIN},
};
use hassle_rs::{Dxc, DxcIncludeHandler, HassleError};
use std::path::Path;

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

pub fn compile_hlsl(path: &Path, src: &str) -> Result<ShaderPackage, EditorError> {
	let dxc = Dxc::new(None).map_err(move |err| EditorError::ShaderCompilation(err))?;

	let compiler = dxc
		.create_compiler()
		.map_err(move |err| EditorError::ShaderCompilation(err))?;
	let library = dxc
		.create_library()
		.map_err(move |err| EditorError::ShaderCompilation(err))?;

	let compile = |entry_point: &str,
	               target_profile: &str,
	               args: &[&str],
	               defines: &[(&str, Option<&str>)]|
	 -> Result<Vec<u32>, EditorError> {
		let blob = library
			.create_blob_with_encoding_from_str(src)
			.map_err(move |err| EditorError::ShaderCompilation(err))?;

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
					.map_err(move |err| EditorError::ShaderCompilation(err))?;
				Err(EditorError::ShaderCompilation(HassleError::CompileError(
					library
						.get_blob_as_string(&error_blob.into())
						.map_err(move |err| EditorError::ShaderCompilation(err))?,
				)))
			}
			Ok(result) => {
				let result_blob = result
					.get_result()
					.map_err(move |err| EditorError::ShaderCompilation(err))?;

				Ok(result_blob.to_vec())
			}
		}
	};

	let vs_ir = if src.contains(VS_MAIN) {
		Some(compile(VS_MAIN, "vs_6_0", &["-spirv"], &[])?)
	} else {
		None
	};

	let ps_ir = if src.contains(PS_MAIN) {
		Some(compile(PS_MAIN, "ps_6_0", &["-spirv"], &[])?)
	} else {
		None
	};

	Ok(ShaderPackage { vs_ir, ps_ir })
}
