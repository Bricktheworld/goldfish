#ifndef TILE_SIZE
#define TILE_SIZE 16
#endif

struct PointLight
{
	float4 color;
	float3 position;
	float radius;
};

struct VisibleLightIndex
{
	int index;
};

struct CullInfo
{
	float4x4 inverse_proj;
	float4x4 view;
	float4x4 proj;
	uint2 screen_size;
	float z_near;
	uint light_count;
};

struct Frustum
{
	// Left, right, top, bottom frustum planes.
	float3 planes[4];
};

[[vk::binding(0,0)]] StructuredBuffer<PointLight> s_point_lights : register(t0);
[[vk::binding(1,0)]] ConstantBuffer<CullInfo> c_cull_info : register(b0);
[[vk::binding(2,0)]] Texture2D<float> t_depth_prepass : register(t1);
[[vk::binding(3,0)]] RWTexture2D<float4> rw_t_heatmap : register(u0);
// [[vk::binding(3,0)]] RWStructuredBuffer<VisibleLightIndex> rw_sb_visible_light_indices : register(u1);

// Atomic min and max depths that are computed in parallel.
groupshared uint gs_min_depth;
groupshared uint gs_max_depth;
groupshared Frustum gs_frustum;
groupshared uint gs_visible_light_count;

// Convert screen space position to view space position.
// Input should have a w component of 1 most likely.
float4 screen_to_view(float4 screen)
{
	// Screen space is gonna be in raw texel coords.
	// We want clip space which will be x: [-1.0, 1.0] (left-to-right), y: [1.0, -1.0] (top-to-bottom), z: [0.0, 1.0] (nearest-to-furthest)
	//
	// Steps:
	// 1. Normalize to [0.0, 1.0] by dividing be screen size.
	// 2. Multiply by 2.0 to get [0.0, 2.0]
	// 3. Subtract by 1.0 to get [-1.0, 1.0] on _both_ x and y.
	// 4. Multiply y by -1.0 so that the y goes from [1.0, -1.0] instead of [-1.0, 1.0] (top-to-bottom)
	float2 normalized_screen = screen.xy / float2(c_cull_info.screen_size) * 2.0f - float2(1.0f, 1.0f);
	normalized_screen.y *= -1.0f;

	float4 clip = float4(normalized_screen, screen.z, screen.w);

	// We get view space by simply applying inverse projection mat.
	float4 view = mul(c_cull_info.inverse_proj, clip);

	// If you do the math here, the inverse proj matrix only gives us [x/(zn), y/(zn), 1/n, 1/(zn)]
	// So we divide by w and get [x, y, z, 1]
	//
	// Expanded math:
	//         [1/w, 0,   0  , 0][(xw)/z]    [x/z]
	// IProj = [0  , 1/y, 0  , 0][(yw)/z]  = [y/z]
	//         [0  , 0  , 0  , 1][   n/z]    [1  ]
	//         [0  , 0  , 1/n, 0][   1  ]    [1/z]
	//
	// [x/z]            [x]
	// [y/z] / (1/z) =  [y]
	// [1  ]            [z]
	// [1/z]            [1]

	view = view / view.w;

	return view;
}

// Left handed coordinate system, points clockwise.
float3 compute_plane(float3 p0, float3 p1, float3 p2)
{
	float3 v0 = p1 - p0;
	float3 v1 = p2 - p0;

	return normalize(cross(v0, v1));
}

Frustum compute_tile_frustum(uint2 global_invocation_id)
{
	// In view-space, the eye is always at the origin.
	const float3 EYE = float3(0.0, 0.0, 0.0);

	// We compute the 4 corners of the tile.
	float2 screen_corners[4];

	// Top left
	screen_corners[0] = global_invocation_id;

	// Top right
	screen_corners[1] = global_invocation_id + uint2(TILE_SIZE, 0);

	// Bottom left
	screen_corners[2] = global_invocation_id + uint2(0, TILE_SIZE);

	// Bottom right
	screen_corners[3] = global_invocation_id + uint2(TILE_SIZE, TILE_SIZE);

	float3 view_corners[4];

	for (int i = 0; i < 4; i++)
	{
		// The z component here is 0.0 because we have a reverse depth buffer, so to set it at the far plane would be to set it to 0.0.
		// NOTE(Brandon): Because we made this choice, the z component of the view_corners is going to be infinite, since we divide by w
		//                which will be 0.0 after applying the inverse projection matrix.
		view_corners[i] = screen_to_view(float4(screen_corners[i], c_cull_info.z_near / 1.0f, 1.0f)).xyz;
	}

	Frustum result;

	// Left plane
	result.planes[0] = compute_plane(EYE, view_corners[0], view_corners[2]);

	// Right plane
	result.planes[1] = compute_plane(EYE, view_corners[3], view_corners[1]);

	// Top plane
	result.planes[2] = compute_plane(EYE, view_corners[1], view_corners[0]);

	// Bottom plane
	result.planes[3] = compute_plane(EYE, view_corners[2], view_corners[3]);

	return result;
}

bool sphere_inside_frustum(float3 center, float radius, float z_nearest, float z_furthest)
{
		float4 view_space = mul(c_cull_info.view, float4(center, 1.0f));
		float3 position = view_space.xyz / view_space.w;

		if (position.z - radius > z_furthest || position.z + radius < z_nearest)
		{
			return false;
		}

		for (int i = 0; i < 4; i++)
		{
			float3 normal = gs_frustum.planes[i];

			float signed_distance = dot(normal, position);
			if (signed_distance < -radius)
			{
				return false;
			}
		}

		return true;
}

[numthreads(TILE_SIZE, TILE_SIZE, 1)]
void cs_main(uint3 global_invocation_id : SV_DispatchThreadID, uint3 local_invocation_id : SV_GroupThreadID, uint local_invocation_index : SV_GroupIndex)
{
	if (local_invocation_index == 0)
	{
		gs_min_depth = 0xFFFFFFFF;
		gs_max_depth = 0;
		gs_visible_light_count = 0;
	}

	GroupMemoryBarrierWithGroupSync();
	uint2 location = uint2(global_invocation_id.xy);

	float clip_depth = t_depth_prepass[location];
	float view_depth = c_cull_info.z_near / clip_depth;
	uint depth_int = asuint(view_depth);

	InterlockedMin(gs_min_depth, depth_int);
	InterlockedMax(gs_max_depth, depth_int);

	GroupMemoryBarrierWithGroupSync();

	if (local_invocation_index == 0)
	{
		gs_frustum = compute_tile_frustum(location);
	}

	GroupMemoryBarrierWithGroupSync();

	float z_nearest = asfloat(gs_min_depth);
	float z_furthest = asfloat(gs_max_depth);

	const uint THREAD_COUNT = TILE_SIZE * TILE_SIZE;
	uint light_index = local_invocation_index;
	if (light_index < c_cull_info.light_count)
	{
		if (sphere_inside_frustum(s_point_lights[light_index].position, s_point_lights[light_index].radius, z_nearest, z_furthest))
		{
			InterlockedAdd(gs_visible_light_count, 1);
		}
	}

	GroupMemoryBarrierWithGroupSync();


	rw_t_heatmap[location] = float4(0.0f, 0.0f, float(gs_visible_light_count) / float(c_cull_info.light_count), 1.0f);

}
