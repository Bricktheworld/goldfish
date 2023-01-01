struct PointLight
{
	float4 color;
	float4 position;
	float radius;
};

[[vk::binding(0,0)]] RWStructuredBuffer<PointLight> sb_point_lights : register(u0);

[numthreads(16, 1, 1)]
void cs_main(uint3 invocation_id: SV_DispatchThreadID)
{
	uint index = uint(invocation_id.x);
	sb_point_lights[index].radius += 1.0f;
}
