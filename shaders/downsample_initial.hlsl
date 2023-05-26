#include "shared.hlsl"

[[vk::binding(0)]] Texture2D<float3> hdr_texture;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2), vk::image_format("rgba16f")]] RWTexture2D<float4> bloom_texture; 

#include "lib.hlsl"

[[vk::push_constant]]
FilterConstants filter_constants;

[numthreads(8, 8, 1)]
void downsample_initial(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(bloom_texture);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    float3 sampled = sample_13_tap_box_filter(hdr_texture, samp, uv, texel_size);

    float3 thresholded = quadratic_colour_thresholding(sampled, filter_constants.threshold, filter_constants.knee);

    bloom_texture[id.xy] = float4(thresholded, 1);
}
