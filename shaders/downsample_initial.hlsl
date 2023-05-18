[[vk::binding(0)]] Texture2D<float3> hdr_texture;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2), vk::image_format("rgba16f")]] RWTexture2D<float4> bloom_texture; 

#include "lib.hlsl"
#include "shared.hlsl"

[[vk::push_constant]]
FilterConstants constants;

[numthreads(8, 8, 1)]
void downsample_initial(
    uint3 id: SV_DispatchThreadID
) {
    float width;
    float height;
    bloom_texture.GetDimensions(width, height);
    float2 texel_size = 1.0 / float2(width, height);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    float3 sampled = sample_13_tap_box_filter(hdr_texture, uv, texel_size);

    float3 thresholded = quadratic_colour_thresholding(sampled, constants.threshold, constants.knee);

    bloom_texture[id.xy] = float4(thresholded, 1);
}
