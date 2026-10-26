struct Frame {
    view_proj: mat4x4<f32>,
    // xyz: direction towards the light.
    light: vec4<f32>,
    // x: 1 when textures are shown.
    flags: vec4<u32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_lit(in: VertexOut) -> @location(0) vec4<f32> {
    var base = in.color;
    let texel = textureSample(tex, samp, in.uv);
    if frame.flags.x == 1u {
        base = base * texel;
    }
    // Pixel art cut-outs: transparent texels leave holes rather than blend.
    if base.a < 0.5 {
        discard;
    }
    let shade = 0.2 + 0.8 * max(dot(normalize(in.normal), frame.light.xyz), 0.0);
    return vec4<f32>(base.rgb * shade, 1.0);
}

@fragment
fn fs_flat(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
