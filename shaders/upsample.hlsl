#include "shared.hlsl"
#include "lib.hlsl"

[[vk::binding(0)]] Texture2D<float3> source_tex;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2), vk::image_format("rgba16f")]] RWTexture2D<float4> dest_tex; 

[numthreads(8, 8, 1)]
void upsample(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(dest_tex);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    dest_tex[id.xy] = float4(dest_tex[id.xy].rgb + sample_3x3_tent_filter(source_tex, samp, uv, texel_size), 1);
}
