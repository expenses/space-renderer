#include "shared.hlsl"

[[vk::binding(0)]] Texture2D<float> depth_tex;
[[vk::binding(1)]] SamplerState samp;
[[vk::binding(2)]] Texture2D<float3> hdr_tex;
[[vk::binding(3), vk::image_format("rgba16f")]] RWTexture2D<float4> output_tex; 

static const float GOLDEN_ANGLE = 2.39996323; 
static const float MAX_BLUR_SIZE = 20.0; 
static const float RAD_SCALE = 3.0; // Smaller = nicer blur, larger = faster

float getBlurSize(float depth, float focusPoint, float focusScale)
{
	float coc = clamp((1.0 / focusPoint - 1.0 / depth)*focusScale, -1.0, 1.0);
	return abs(coc) * MAX_BLUR_SIZE;
}

[numthreads(8, 8, 1)]
void compute_dof(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(output_tex);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    // https://www.reddit.com/r/GraphicsProgramming/comments/f9zwin/linearising_reverse_depth_buffer/fix7ifb/
    float near_depth = 0.001;
    float center_depth = near_depth / depth_tex.SampleLevel(samp, uv, 0).r;

    float focus_point = near_depth / depth_tex.SampleLevel(samp, float2(0.5, 0.5), 0).r;
    float focusScale = 1.0;

	float center_size = getBlurSize(center_depth, focus_point, focusScale);

    float3 colour = hdr_tex.SampleLevel(samp, uv, 0);
    float tot = 1.0;
	float radius = RAD_SCALE;
	for (float ang = 0.0; radius<MAX_BLUR_SIZE; ang += GOLDEN_ANGLE)
	{
		float2 tc = uv + float2(cos(ang), sin(ang)) * texel_size * radius;
		float3 sampleColour = hdr_tex.SampleLevel(samp, tc, 0);
		float sampleDepth = near_depth / depth_tex.SampleLevel(samp, tc, 0).r;
		float sampleSize = getBlurSize(sampleDepth, focus_point, focusScale);
		if (sampleDepth > center_depth) {
			sampleSize = clamp(sampleSize, 0.0, center_size*2.0);
        }
		float m = smoothstep(radius-0.5, radius+0.5, sampleSize);
		colour += lerp(colour/tot, sampleColour, m);
		tot += 1.0;
        radius += RAD_SCALE / radius;
	}

    output_tex[id.xy].rgb = colour / tot;
}
