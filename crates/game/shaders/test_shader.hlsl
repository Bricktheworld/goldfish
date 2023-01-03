#include "common.hlsli"

struct PSInput
{
	float4 position : SV_POSITION;
};

PSInput vs_main(VSInput input)
{
	PSInput result;
	
	result.position = mul(c_camera.view_proj, mul(c_model.matrix, float4(input.position, 1.0)));
	
	return result;
}

float4 ps_main(PSInput input) : SV_TARGET
{
	return float4(1.0, 0.0, 0.0, 1.0);
}
