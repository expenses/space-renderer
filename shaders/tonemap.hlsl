#include "shared.hlsl"

[[vk::binding(0), vk::image_format("rgba16f")]] RWTexture2D<float4> hdr_tex;

#include "tony-mc-mapface/shader/tony_mc_mapface.hlsl"

[numthreads(8, 8, 1)]
void tonemap(
    uint3 id: SV_DispatchThreadID
) {
    uint2 output_size = texture_size(hdr_tex);

    if (!(id.x < output_size.x && id.y < output_size.y)) {
        return;
    }

    float3 value = hdr_tex[id.xy].xyz;
    hdr_tex[id.xy].xyz = tony_mc_mapface(value);
}
