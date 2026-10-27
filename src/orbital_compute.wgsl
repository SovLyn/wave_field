struct Params {
    quantum: vec4<u32>, // n, l, m+2, count
    view: vec4<f32>, // yaw, pitch, zoom, aspect
    style: vec4<f32>, // point size, brightness, threshold, color mode
    extra: vec4<f32>, // radial envelope, maximum radial density, viewport width, viewport height
}
struct Particle { position_phase: vec4<f32>, data: vec4<f32> }
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;

fn hash(v: u32) -> u32 {
    var x = v;
    x = ((x >> 16u) ^ x) * 0x7feb352du;
    x = ((x >> 15u) ^ x) * 0x846ca68bu;
    return (x >> 16u) ^ x;
}
fn random(s: ptr<function, u32>) -> f32 {
    *s = hash(*s + 0x9e3779b9u);
    return f32(*s >> 8u) * (1.0 / 16777216.0);
}
// Common normalization factors are irrelevant to rejection sampling within one orbital.
fn radial(r: f32) -> f32 {
    let n = p.quantum.x; let l = p.quantum.y;
    if n == 1u { return exp(-r); }
    if n == 2u {
        if l == 0u { return (2.0-r)*exp(-r/2.0); }
        return r*exp(-r/2.0);
    }
    if l == 0u { return (27.0-18.0*r+2.0*r*r)*exp(-r/3.0); }
    if l == 1u { return r*(6.0-r)*exp(-r/3.0); }
    return r*r*exp(-r/3.0);
}
// Real tesseral harmonics, scaled so their absolute maximum is one.
fn angular(d: vec3<f32>) -> f32 {
    let l = p.quantum.y; let m = i32(p.quantum.z)-2;
    if l == 0u { return 1.0; }
    if l == 1u {
        if m == -1 { return d.y; }
        if m == 0 { return d.z; }
        return d.x;
    }
    if m == -2 { return 2.0*d.x*d.y; }
    if m == -1 { return 2.0*d.y*d.z; }
    if m == 0 { return (3.0*d.z*d.z-1.0)*0.5; }
    if m == 1 { return 2.0*d.x*d.z; }
    return d.x*d.x-d.y*d.y;
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= p.quantum.w { return; }
    var seed = hash(id.x + p.quantum.x*0x1f123bb5u + p.quantum.y*0x5f356495u + p.quantum.z*0x6c8e9cf5u);
    var r = 0.0; var found_r = false;
    // Finite support [0,40] Bohr radii, sampled with the r² Jacobian.
    for (var i=0u; i<256u; i++) {
        let candidate = 40.0*random(&seed);
        let f = radial(candidate); let weight = candidate*candidate*f*f;
        if random(&seed)*p.extra.x <= weight { r=candidate; found_r=true; break; }
    }
    var d = vec3<f32>(0.0,0.0,1.0); var a = 0.0; var found_a = false;
    for (var i=0u; i<64u; i++) {
        let z = 2.0*random(&seed)-1.0; let phi = 6.28318530718*random(&seed);
        let xy = sqrt(max(0.0,1.0-z*z));
        let candidate = vec3<f32>(xy*cos(phi),xy*sin(phi),z);
        let h = angular(candidate);
        if random(&seed) <= h*h { d=candidate; a=h; found_a=true; break; }
    }
    let f = radial(r);
    // Local density relative to a conservative orbital-specific radial bound.
    let density = clamp(f*f*a*a / max(p.extra.y, 1e-20), 0.0, 1.0);
    particles[id.x].position_phase = vec4<f32>(d*r, select(-1.0, 1.0, f*a >= 0.0));
    particles[id.x].data = vec4<f32>(density, r, select(0.0,1.0,found_r && found_a), 0.0);
}
