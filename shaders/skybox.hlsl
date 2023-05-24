struct PushConstant {
    float boost;
};

[[vk::binding(2)]]
cbuffer Matrices {
    float4x4 projection_inverse;
    float4x4 view;
};

[[vk::push_constant]]
PushConstant constant;

struct Varying {
    float4 position: SV_Position;
    float3 ray: TEXCOORD0;
};

[shader("vertex")]
Varying VSMain(
    uint v_id : SV_VertexID
) {
    float4 pos = float4(
        float(v_id / 2) * 4 - 1,
        float(v_id & 1) * 4 - 1,
        0.0,
        1.0
    );

    float4 unprojected = mul(projection_inverse, pos);
    float3 ray = mul(view, float4(unprojected.xyz, 1.0)).xyz; 

    Varying output;
    output.position = pos;
    output.ray = ray;
    return output;
}

[[vk::binding(0)]] TextureCube<float3> tex;
[[vk::binding(1)]] SamplerState samp;

[shader("pixel")]
float4 PSMain(
    float3 ray: TEXCOORD0
): SV_Target0 {
    return float4(tex.Sample(samp, ray) * constant.boost, 1.0);
}
