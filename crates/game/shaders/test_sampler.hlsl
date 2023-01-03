#include "common.hlsli"

struct PSInput
{
	float4 position : SV_POSITION;
	float2 uv : TEXCOORD0;
};

[[vk::binding(0,1)]] Texture2D<float4> t_albedo : register(t0);
[[vk::binding(1,1)]] SamplerState s_albedo : register(s1);

PSInput vs_main(VSInput input)
{
	PSInput result;
	
	result.position = mul(c_camera.view_proj, mul(c_model.matrix, float4(input.position, 1.0)));
	result.uv = input.uv;
	
	return result;
}

float4 ps_main(PSInput input) : SV_TARGET
{
	return t_albedo.Sample(s_albedo, input.uv);
}
