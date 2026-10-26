@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return VertexOut(vec4<f32>(corners[index], 0.0, 1.0));
}

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// GPUI's surface shader expects limited-range BT.709 YCbCr.
@fragment
fn fs_y(in: VertexOut) -> @location(0) f32 {
    let size = vec2<f32>(textureDimensions(source));
    let color = textureSample(source, source_sampler, in.position.xy / size).rgb;
    return (16.0 + 219.0 * luma(color)) / 255.0;
}

@fragment
fn fs_chroma(in: VertexOut) -> @location(0) vec2<f32> {
    let size = vec2<f32>(textureDimensions(source));
    let color = textureSample(source, source_sampler, in.position.xy * 2.0 / size).rgb;
    let y = luma(color);
    let cb = (color.b - y) / 1.8556;
    let cr = (color.r - y) / 1.5748;
    return vec2<f32>(128.0 / 255.0 + 224.0 / 255.0 * cb, 128.0 / 255.0 + 224.0 / 255.0 * cr);
}
