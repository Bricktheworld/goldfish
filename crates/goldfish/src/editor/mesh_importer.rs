use super::EditorError;
use glam::{vec2, vec3};
use goldfish::{package::MeshPackage, renderer::Vertex};
use russimp::scene::{PostProcess, Scene};

pub fn import_mesh(data: &[u8], extension: &str) -> Result<Vec<MeshPackage>, EditorError> {
	let scene = Scene::from_buffer(
		data,
		vec![
			PostProcess::CalculateTangentSpace,
			PostProcess::Triangulate,
			PostProcess::JoinIdenticalVertices,
			PostProcess::SortByPrimitiveType,
			PostProcess::MakeLeftHanded,
		],
		extension,
	)
	.map_err(move |err| EditorError::MeshImport(err))?;
	Ok(scene
		.meshes
		.iter()
		.map(|mesh| {
			let vertices = (0..mesh.vertices.len())
				.map(|i| Vertex {
					position: vec3(mesh.vertices[i].x, mesh.vertices[i].y, mesh.vertices[i].z),
					normal: vec3(mesh.normals[i].x, mesh.normals[i].y, mesh.normals[i].z),
					tangent: vec3(mesh.tangents[i].x, mesh.tangents[i].y, mesh.tangents[i].z),
					uv: if let Some(ref uv) = mesh.texture_coords[0] { vec2(uv[i].x, uv[i].y) } else { vec2(0.0, 0.0) },
					bitangent: vec3(mesh.bitangents[i].x, mesh.bitangents[i].y, mesh.bitangents[i].z),
				})
				.collect::<Vec<_>>();

			let indices = mesh
				.faces
				.iter()
				.flat_map(|face| {
					assert_eq!(face.0.len(), 3, "Invalid number of indices!");
					[face.0[0] as u16, face.0[1] as u16, face.0[2] as u16]
				})
				.collect::<Vec<u16>>();

			MeshPackage { vertices, indices }
		})
		.collect::<Vec<_>>())
}
