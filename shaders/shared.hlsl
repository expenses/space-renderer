
struct FilterConstants {
    float threshold;
    float knee;
};

// Compute shader thread IDs are always i32 in wgsl, so we need to use
// i32 texture sizes to check against to avoid getting naga errors.
template<typename T>
int2 texture_size(T texture) {
    uint width;
    uint height;
    texture.GetDimensions(width, height);
    return uint2(width, height);
}
