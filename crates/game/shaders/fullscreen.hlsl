[[vk::binding(0,0)]] Texture2D<float4> t_input : register(t0);
[[vk::binding(1,0)]] SamplerState s_input : register(s0);

struct PSInput
{
	float4 position : SV_POSITION;
	float2 uv : TEXCOORD0;
};

PSInput vs_main(uint vert_id : SV_VertexID)
{
	PSInput result;

	// WHAT WE WANT
	// float2(0.0f, 0.0f) -> float2(-1.0f, 1.0f)
	// float2(2.0f, 0.0f) -> float2(3.0f, 1.0f)
	// float2(0.0f, 2.0f) -> float2(-1.0f, -3.0f)

	result.uv = float2((vert_id << 1) & 2, vert_id & 2);
	result.position = float4(result.uv.x * 2.0f - 1.0f, result.uv.y * -2.0f + 1.0f, 0.0f, 1.0f);

	return result;
}

float4 ps_main (PSInput input) : SV_TARGET
{
	return t_input.Sample(s_input, input.uv);
}

