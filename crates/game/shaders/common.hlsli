#ifndef COMMON
#define COMMON

#define CAMERA_BUFFER_SLOT b0

struct VS_INPUT
{
    float3 position : POSITION;
    float3 normal : NORMAL;
    float2 uv : UV;
    float3 tangent: TANGENT;
    float3 bitangent: BITANGENT;
};

struct Camera
{
	float3 position;
	float4x4 view;
	float4x4 proj;
	float4x4 viewProj;
};

[[vk::binding(0,0)]] ConstantBuffer<Camera> gCamera : register(CAMERA_BUFFER_SLOT);

struct Model
{
	float4x4 matrix;
};
[[vk::binding(1,0)]] ConstantBuffer<Model> gModel : register(b1);

struct PushConstants
{
	float4x4 Model;
};

// [[vk::push_constant]] PushConstants gPushConstants;

#endif
