[[vk::binding(0,0)]] Texture2D<float4> t_depth : register(t0);
[[vk::binding(1,0)]] SamplerState s_depth : register(s0);

struct NearPlane
{
	float z_near;
};

[[vk::binding(2,0)]] ConstantBuffer<NearPlane> c_near : register(b0);

struct PSInput
{
	float4 position : SV_POSITION;
	float2 uv : TEXCOORD0;
};

#define Z_NEAR (0.01f)

float4 ps_main (PSInput input) : SV_TARGET
{
	float depth = t_depth.Sample(s_depth, input.uv).r;

	// TODO(Brandon): I have no idea what I'm doing wrong, but it's nearly impossible to see the depth buffer if I don't divide the linearization by 10...
	float linearized = Z_NEAR / depth * 0.1f;
	return float4(linearized, linearized, linearized, 1.0f);
}

