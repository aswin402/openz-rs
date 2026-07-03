@group(0) @binding(0) var<storage, read_write> pixels: array<u32>;
@group(0) @binding(1) var<uniform> dimensions: vec4<u32>; // x: width, y: height

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dimensions.x || id.y >= dimensions.y) {
        return;
    }
    let idx = id.y * dimensions.x + id.x;
    let pixel = pixels[idx];
    
    let r = 255u - ((pixel >> 0u) & 0xffu);
    let g = 255u - ((pixel >> 8u) & 0xffu);
    let b = 255u - ((pixel >> 16u) & 0xffu);
    let a = (pixel >> 24u) & 0xffu;
    
    pixels[idx] = r | (g << 8u) | (b << 16u) | (a << 24u);
}
