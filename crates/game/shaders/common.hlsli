#ifndef COMMON
#define COMMON
#include "utils.hlsli"

#define CAMERA_BUFFER_SLOT b9999
#define MODEL_BUFFER_SLOT b9998

struct VSInput
{
    float3 position : POSITION0;
    float3 normal : NORMAL0;
    float2 uv : TEXCOORD0;
    float3 tangent: TANGENT0;
    float3 bitangent: BINORMAL0;
};

struct Camera
{
	float3 position;
	float4x4 view;
	float4x4 proj;
	float4x4 view_proj;
};

[[vk::binding(0,0)]] ConstantBuffer<Camera> c_camera : register(CAMERA_BUFFER_SLOT);

struct Model
{
	float4x4 matrix;
};
[[vk::binding(1,0)]] ConstantBuffer<Model> c_model : register(MODEL_BUFFER_SLOT);

#endif
