struct PushConstant {
    float4x4 combined_matrix;
};

[[vk::push_constant]]
PushConstant constant;

struct Varying {
    float4 position: SV_Position;
    float2 uv: TEXCOORD0;
    float3 normal: NORMAL0;
};

[shader("vertex")]
Varying VSMain(
    float3 position: POSITION,
    float2 uv: TEXCOORD0,
    float3 normal: NORMAL0
) {
    Varying output;
    output.position = mul(constant.combined_matrix, float4(position, 1.0));
    output.uv = uv;
    output.normal = normal;
    return output;
}

[[vk::binding(0)]] Texture2D<float3> tex;
[[vk::binding(1)]] SamplerState samp;

[shader("pixel")]
float4 PSMain(
    Varying varying
): SV_Target0 {
    float brightness = max(dot(normalize(varying.normal), normalize(float3(1,1,1))), 0.5);

    return float4(tex.Sample(samp, varying.uv) * brightness * 10.0, 1.0);
}
