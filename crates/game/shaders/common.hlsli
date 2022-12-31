#ifndef COMMON
#define COMMON

#define CAMERA_BUFFER_SLOT b0

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

[[vk::binding(0,0)]] ConstantBuffer<Camera> g_camera : register(CAMERA_BUFFER_SLOT);

struct Model
{
	float4x4 matrix;
};
[[vk::binding(1,0)]] ConstantBuffer<Model> g_model : register(b1);

struct PushConstants
{
	float4x4 model;
};

// [[vk::push_constant]] PushConstants g_pushConstants;

#endif
