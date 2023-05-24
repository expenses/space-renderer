#include "quaternion.hlsl"

struct PushConstant {
    float4x4 combined_matrix;
    float3 camera_pos;
};

static const uint INVALID = 4294967295;

struct MaterialInfo {
    float4 base_color_factor;
    float3 emissive_factor;
    float metallic_factor;
    float roughness_factor;
    uint albedo_texture;
    uint normal_texture;
    uint emissive_texture;
};

[[vk::push_constant]]
PushConstant constant;

struct Varying {
    float4 builtin_position: SV_Position;
    float3 position: POSITION0;
    float2 uv: TEXCOORD0;
    float3 normal: NORMAL0;
    uint material_id: TEXCOORD1;
};

[shader("vertex")]
Varying VSMain(
    float3 position: POSITION,
    float2 uv: TEXCOORD0,
    float3 normal: NORMAL0,
    uint material_id: TEXCOORD1,
    float3 instance_position: TEXCOORD2,
    float instance_scale: TEXCOORD3,
    float4 instance_rotation: TEXCOORD4
) {
    Similarity transform = Similarity::from(instance_position, Quaternion::from_float4(instance_rotation), instance_scale);
    Varying output;
    output.position = transform * position;
    output.builtin_position = mul(constant.combined_matrix, float4(output.position, 1.0));
    output.uv = uv;
    output.material_id = material_id;
    output.normal = transform.rotation * normal;
    return output;
}


[[vk::binding(0)]] Texture2D<float3> tex[];
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2)]] StructuredBuffer<MaterialInfo> infos;

float length_squared(float3 vec) {
    return dot(vec, vec);
}

float3x3 compute_cotangent_frame(
    float3 normal,
    float3 position,
    float2 uv
) {
    float3 delta_pos_x = ddx(position);
    float3 delta_pos_y = ddy(position);
    float2 delta_uv_x = ddx(uv);
    float2 delta_uv_y = ddy(uv);

    float3 delta_pos_y_perp = cross(delta_pos_y, normal);
    float3 delta_pos_x_perp = cross(normal, delta_pos_x);

    float3 t = delta_pos_y_perp * delta_uv_x.x + delta_pos_x_perp * delta_uv_y.x;
    float3 b = delta_pos_y_perp * delta_uv_x.y + delta_pos_x_perp * delta_uv_y.y;

    float invmax = 1.0 / sqrt(max(length_squared(t), length_squared(b)));
    return transpose(float3x3(t * invmax, b * invmax, normal));
}
    

[shader("pixel")]
float4 PSMain(
    Varying varying
): SV_Target0 {
    MaterialInfo info = infos[varying.material_id];

    float3 normal = normalize(varying.normal);

    float3 view_vector = constant.camera_pos - varying.position;

    if (info.normal_texture != INVALID) {
        float3 tex_normal = tex[info.normal_texture].Sample(samp, varying.uv).xyz * 255.0 / 127.0 - 128.0 / 127.0;
        normal = normalize(mul(compute_cotangent_frame(normal, -view_vector, varying.uv), tex_normal));
    }

    float3 sun_dir = normalize(float3(1,1,1));
    float brightness = max(dot(normal, sun_dir), 0.0);

    float3 albedo = info.base_color_factor.rgb;

    if (info.albedo_texture != INVALID) {
        albedo *= tex[info.albedo_texture].Sample(samp, varying.uv).xyz;
    }

    float3 emissive = info.emissive_factor;

    if (info.emissive_texture != INVALID) {
        emissive *= tex[info.emissive_texture].Sample(samp, varying.uv).xyz;
    }

    return float4(albedo * brightness + emissive, 1.0);
}
