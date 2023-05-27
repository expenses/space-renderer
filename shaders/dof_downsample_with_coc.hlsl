#include "shared.hlsl"

[[vk::binding(0)]] Texture2D<float> depth_tex;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2)]] Texture2D<float3> hdr_tex;
[[vk::binding(3), vk::image_format("rgba16f")]] RWTexture2D<float4> output_tex; 
[[vk::binding(4)]] SamplerState samp_non_filtering;

float get_coc(float depth, float focusPoint)
{
	return clamp((1.0 / focusPoint - 1.0 / depth), -1.0, 1.0);
}

[numthreads(8, 8, 1)]
void dof_downsample_with_coc(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(output_tex);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    float3 colour = hdr_tex.SampleLevel(samp, uv, 0);

    // https://www.reddit.com/r/GraphicsProgramming/comments/f9zwin/linearising_reverse_depth_buffer/fix7ifb/
    float near_depth = 0.001;
    float center_depth = near_depth / depth_tex.SampleLevel(samp_non_filtering, uv, 0).r;

    float focus_point = near_depth / depth_tex.SampleLevel(samp_non_filtering, float2(0.5, 0.5), 0).r;

    output_tex[id.xy] = float4(colour, get_coc(center_depth, focus_point));
}
