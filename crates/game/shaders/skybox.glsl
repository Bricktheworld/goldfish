#[UNIFORMS]

layout (set = 1, binding = 0) uniform Skybox {
    vec4 unused;
} u_Skybox;
layout (set = 1, binding = 1) uniform samplerCube u_Skybox_map;

#[VERTEX]
layout (location = 0) out vec3 f_UV;

void main()
{
    f_UV = v_Position;
		vec4 pos = u_Camera.proj * mat4(mat3(u_Camera.view)) * vec4(v_Position, 1.0f);
    gl_Position = pos.xyww;
}

#[FRAGMENT]

layout (location = 0) out vec4 color;

layout (location = 0) in vec3 f_UV;

void main()
{
    color = texture(u_Skybox_map, f_UV);
}
