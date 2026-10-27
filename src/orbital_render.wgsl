struct Params {
    quantum: vec4<u32>, view: vec4<f32>, style: vec4<f32>, extra: vec4<f32>,
}
struct Particle { position_phase: vec4<f32>, data: vec4<f32> }
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) visible: f32,
}
@vertex
fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Out {
    let q = particles[instance];
    // Fit different shells into the view without changing their relative shapes.
    // Portrait viewports use the narrower dimension to prevent horizontal clipping.
    let shell_scale = select(select(0.55, 1.0, p.quantum.x == 2u), 3.0, p.quantum.x == 1u);
    let pos = q.position_phase.xyz * shell_scale * min(p.view.w, 1.0);
    let cy=cos(p.view.x); let sy=sin(p.view.x);
    let cp=cos(p.view.y); let sp=sin(p.view.y);
    let x=cy*pos.x+sy*pos.z;
    let z=-sy*pos.x+cy*pos.z;
    let y=cp*pos.y-sp*z;
    let depth=sp*pos.y+cp*z;
    let distance = 65.0/max(p.view.z,0.05);
    let w = distance-depth;
    let aspect=max(p.view.w,0.001);
    let corners = array<vec2<f32>,6>(vec2<f32>(-1.0,-1.0),vec2<f32>(1.0,-1.0),vec2<f32>(-1.0,1.0),vec2<f32>(-1.0,1.0),vec2<f32>(1.0,-1.0),vec2<f32>(1.0,1.0));
    let uv=corners[vertex];
    // The quad is specified in physical pixels; perspective shrinks distant points.
    let size = p.style.x * clamp(distance/max(w,1.0),0.15,4.0);
    let offset = uv * size * 2.0 / max(p.extra.zw,vec2<f32>(1.0));
    var out: Out;
    out.position=vec4<f32>(x*2.4/aspect + offset.x*w, y*2.4 + offset.y*w, max(w*0.5,0.0), w);
    out.uv=uv;
    let mode=u32(p.style.w);
    if mode == 0u {
        out.color=select(vec3<f32>(0.32,0.65,1.0),vec3<f32>(1.0,0.48,0.32),q.position_phase.w < 0.0);
    } else if mode == 1u {
        let d=clamp(sqrt(q.data.x)*4.0,0.0,1.0);
        out.color=mix(vec3<f32>(0.15,0.38,0.9),vec3<f32>(0.85,0.95,1.0),d);
    } else {
        let t=clamp(q.data.y/30.0,0.0,1.0);
        out.color=mix(vec3<f32>(0.3,0.85,1.0),vec3<f32>(0.8,0.32,0.9),t);
    }
    // On an 8-bit additive target, very small contributions quantize to zero.
    // Instead, thin the displayed points deterministically at high count/small
    // viewports; all requested samples are still generated and retained on GPU.
    let count_scale = min(1.0, 300000.0 / max(f32(p.quantum.w), 1.0));
    let viewport_scale = min(1.0, pow(min(p.extra.z, p.extra.w) / 900.0, 2.0));
    let pick = f32((instance * 1664525u + 1013904223u) >> 8u) / 16777216.0;
    out.visible=select(0.0,1.0,q.data.z > 0.5 && q.data.x >= p.style.z && w > 0.1 && pick < count_scale * viewport_scale);
    return out;
}
@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    let radius=dot(in.uv,in.uv);
    if radius > 1.0 || in.visible < 0.5 { discard; }
    let glow=exp(-4.0*radius)*p.style.y*0.025;
    return vec4<f32>(in.color*glow,0.0);
}
