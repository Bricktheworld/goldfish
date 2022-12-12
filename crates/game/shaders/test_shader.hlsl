#include "common.hlsli"

struct PSInput
{
	float4 position : SV_POSITION;
};

PSInput vs_main(VS_INPUT input)
{
	PSInput result;
	
	result.position = mul(gCamera.viewProj, mul(gModel.matrix, float4(input.position, 1.0)));
	
	return result;
}

float4 ps_main(PSInput input) : SV_TARGET
{
	return float4(1.0, 0.0, 0.0, 1.0);
}
