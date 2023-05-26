#include "dof_filters.hlsl"
#include "shared.hlsl"
#include "dof_values.hlsl"

[[vk::binding(0)]] SamplerState samp;
[[vk::binding(1)]] Texture2D<float4> hdr_and_coc;
// Naga requires that all storage texture writes are to float4 textures.
[[vk::binding(2), vk::image_format("rg16f")]] RWTexture2DArray<float4> output;

[numthreads(8, 8, 1)]
void dof_x(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(hdr_and_coc);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float2 texel_size = 1.0 / float2(output_size);
    float2 uv = (float2(id.xy) + 0.5) * texel_size;

    Values values = DEFAULT_VALUES;

    float filterRadius = hdr_and_coc.SampleLevel(samp, uv, 0).a;
    for (int i = 0; i < KERNEL_COUNT; i++) {
        float2 coords = uv + texel_size * float2(float(i - int(KERNEL_RADIUS)),0.0) * filterRadius;
        float3 rgb = hdr_and_coc.SampleLevel(samp, coords, 0).rgb;
        
        float2 c0 = Kernel0_RealX_ImY_RealZ_ImW[i].xy;
        
        values.red_0 += rgb.r * c0;
        values.green_0 += rgb.g * c0;
        values.blue_0 += rgb.b * c0;
        
        float2 c1 = Kernel1_RealX_ImY_RealZ_ImW[i].xy;
        
        values.red_1 += rgb.r * c1;   
        values.green_1 += rgb.g * c1;
        values.blue_1 += rgb.b * c1;

        float2 c2 = Kernel2_RealX_ImY_RealZ_ImW[i].xy;

        values.red_2 += rgb.r * c2;     
        values.green_2 += rgb.g * c2;     
        values.blue_2 += rgb.b * c2;   
    }

    output[uint3(id.xy, 0)].xy = values.red_0;
    output[uint3(id.xy, 1)].xy = values.green_0;
    output[uint3(id.xy, 2)].xy = values.blue_0;
    output[uint3(id.xy, 3)].xy = values.red_1;
    output[uint3(id.xy, 4)].xy = values.green_1;
    output[uint3(id.xy, 5)].xy = values.blue_1;
    output[uint3(id.xy, 6)].xy = values.red_2;
    output[uint3(id.xy, 7)].xy = values.green_2;
    output[uint3(id.xy, 8)].xy = values.blue_2;
}
