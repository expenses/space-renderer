#include "shared.hlsl"

[[vk::binding(0)]] Texture2D<float4> source_tex;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2), vk::image_format("rgba16f")]] RWTexture2D<float4> output;

[numthreads(8, 8, 1)]
void blit_compute(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(output);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    float4 value = source_tex.SampleLevel(samp, uv, 0);

    output[id.xy] = value;
}
