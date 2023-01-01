[[vk::binding(0,0)]] Texture2D<float4> t_depth : register(t0);
[[vk::binding(1,0)]] SamplerState s_depth : register(s0);

struct NearPlane
{
	float z_near;
	float z_scale;
};

[[vk::binding(2,0)]] ConstantBuffer<NearPlane> c_near : register(b0);

struct PSInput
{
	float4 position : SV_POSITION;
	float2 uv : TEXCOORD0;
};

float4 ps_main (PSInput input) : SV_TARGET
{
	float depth = t_depth.Sample(s_depth, input.uv).r;

	float linearized = c_near.z_near / depth * c_near.z_scale;
	return float4(linearized, linearized, linearized, 1.0f);
}

