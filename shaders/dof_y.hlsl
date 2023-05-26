#include "dof_filters.hlsl"
#include "shared.hlsl"
#include "dof_values.hlsl"

[[vk::binding(0)]] SamplerState samp;
[[vk::binding(1)]] Texture2D<float4> hdr_tex;
[[vk::binding(2)]] Texture2DArray<float2> horizontally_blurred;
[[vk::binding(3), vk::image_format("rgba16f")]] RWTexture2D<float4> out_tex;

//(Pr+Pi)*(Qr+Qi) = (Pr*Qr+Pr*Qi+Pi*Qr-Pi*Qi)
float2 multComplex(float2 p, float2 q)
{
    return float2(p.x*q.x-p.y*q.y, p.x*q.y+p.y*q.x);
}

[numthreads(8, 8, 1)]
void dof_y(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(hdr_tex);

	if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    Values values = DEFAULT_VALUES;
    
    float filterRadius = hdr_tex.SampleLevel(samp, uv, 0).a;
    
    for (int i = 0; i < KERNEL_COUNT; i++) {
        float2 coords = uv + texel_size*float2(0.0,float(i - int(KERNEL_RADIUS)))*filterRadius;
        
        float2 red_0    = horizontally_blurred.SampleLevel(samp, float3(coords, 0), 0);
        float2 green_0  = horizontally_blurred.SampleLevel(samp, float3(coords, 1), 0);
        float2 blue_0   = horizontally_blurred.SampleLevel(samp, float3(coords, 2), 0);
        float2 c0 = Kernel0_RealX_ImY_RealZ_ImW[i].xy;
        
        values.red_0 += multComplex(red_0, c0);
        values.green_0 += multComplex(green_0, c0);
        values.blue_0 += multComplex(blue_0, c0);
        
        float2 red_1    = horizontally_blurred.SampleLevel(samp, float3(coords, 3), 0);
        float2 green_1  = horizontally_blurred.SampleLevel(samp, float3(coords, 4), 0);
        float2 blue_1   = horizontally_blurred.SampleLevel(samp, float3(coords, 5), 0);
        float2 c1 = Kernel1_RealX_ImY_RealZ_ImW[i].xy;

        values.red_1 += multComplex(red_1, c1);
        values.green_1 += multComplex(green_1, c1);
        values.blue_1 += multComplex(blue_1, c1);
        
        float2 red_2    = horizontally_blurred.SampleLevel(samp, float3(coords, 6), 0);
        float2 green_2  = horizontally_blurred.SampleLevel(samp, float3(coords, 7), 0);
        float2 blue_2   = horizontally_blurred.SampleLevel(samp, float3(coords, 8), 0);
        float2 c2 = Kernel2_RealX_ImY_RealZ_ImW[i].xy;

        values.red_2 += multComplex(red_2, c2);
        values.green_2 += multComplex(green_2, c2);
        values.blue_2 += multComplex(blue_2, c2);
    }
    
    float2 w0 = Kernel0Weights_RealX_ImY;
    float2 w1 = Kernel1Weights_RealX_ImY;
    float2 w2 = Kernel2Weights_RealX_ImY;
    //float2 w3 = Kernel3Weights_RealX_ImY;

    float red   = dot(values.red_0, w0) + dot(values.red_1, w1) + dot(values.red_2, w2);// + dot(values.red_3, w3);
    float green = dot(values.green_0, w0) + dot(values.green_1, w1) + dot(values.green_2, w2);// + dot(values.green_3, w3);
    float blue  = dot(values.blue_0, w0) + dot(values.blue_1, w1) + dot(values.blue_2, w2);// + dot(values.blue_3, w3);
       
    out_tex[id.xy] = float4(red, green, blue, 1.0);
}
