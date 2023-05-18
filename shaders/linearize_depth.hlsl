[[vk::binding(0)]] Texture2D<float> depth_tex;
[[vk::binding(1)]] SamplerState samp;
//[[vk::binding(2)]] Texture2D<float3> hdr_tex;
[[vk::binding(2), vk::image_format("r16f")]] RWTexture2D<float4> output_tex; 

static const float GOLDEN_ANGLE = 2.39996323; 
static const float MAX_BLUR_SIZE = 20.0; 
static const float RAD_SCALE = 0.5; // Smaller = nicer blur, larger = faster

float getBlurSize(float depth, float focusPoint, float focusScale)
{
	float coc = clamp((1.0 / focusPoint - 1.0 / depth)*focusScale, -1.0, 1.0);
	return abs(coc) * MAX_BLUR_SIZE;
}

[numthreads(8, 8, 1)]
void linearize_depth(
    uint3 id: SV_DispatchThreadID
) {
    float width;
    float height;
    output_tex.GetDimensions(width, height);
    float2 texel_size = 1.0 / float2(width, height);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    // https://www.reddit.com/r/GraphicsProgramming/comments/f9zwin/linearising_reverse_depth_buffer/fix7ifb/
    float near_depth = 0.001;
    float center_depth = near_depth / depth_tex.SampleLevel(samp, uv, 0).r;

    float focusPoint = 10.0;
    float focusScale = 1.0;

	float center_size = getBlurSize(center_depth, focusPoint, focusScale);

    /*float3 colour = hdr_tex.SampleLevel(samp, uv, 0);
    float tot = 1.0;
	float radius = RAD_SCALE;
	for (float ang = 0.0; radius<MAX_BLUR_SIZE; ang += GOLDEN_ANGLE)
	{
		float2 tc = uv + float2(cos(ang), sin(ang)) * texel_size * radius;
		float3 sampleColour = hdr_tex.SampleLevel(samp, tc, 0);
		float sampleDepth = near_depth / depth_tex.SampleLevel(samp, tc, 0).r;
		float sampleSize = getBlurSize(sampleDepth, focusPoint, focusScale);
		if (sampleDepth > center_depth) {
			sampleSize = clamp(sampleSize, 0.0, center_size*2.0);
        }
		float m = smoothstep(radius-0.5, radius+0.5, sampleSize);
		colour += lerp(colour/tot, sampleColour, m);
		tot += 1.0;
        radius += RAD_SCALE/radius;
	}*/

    output_tex[id.xy].x = center_size;
}
