struct Camera {
    view_proj: mat4x4<f32>,
}

struct Joints {
    m: array<mat4x4<f32>, 64>,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> joints: Joints;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs(in: VertexIn) -> VertexOut {
    let skin = joints.m[in.joints.x] * in.weights.x
        + joints.m[in.joints.y] * in.weights.y
        + joints.m[in.joints.z] * in.weights.z
        + joints.m[in.joints.w] * in.weights.w;

    let world = skin * vec4<f32>(in.position, 1.0);
    let shade = 0.35 + 0.65 * abs(normalize((skin * vec4<f32>(in.normal, 0.0)).xyz).y);

    var out: VertexOut;
    out.clip = camera.view_proj * world;
    out.color = in.color * shade;
    return out;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
