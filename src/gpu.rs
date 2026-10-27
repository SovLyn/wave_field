//! WebGPU hydrogen cloud. Call `setup` once on egui's wgpu callback resources,
//! then add one `callback` per frame to the painter. One cloud callback per frame
//! is supported: the GPU buffers and uniforms are shared by the renderer.
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};

#[derive(Clone, Copy, Debug)]
pub struct RenderParams {
    pub n: u32,
    pub l: u32,
    pub m: i32,
    pub count: u32,
    pub point_size: f32,
    pub brightness: f32,
    pub color_mode: u32,
    pub threshold: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub time: f32,
}

struct Gpu {
    compute: wgpu::ComputePipeline,
    render: wgpu::RenderPipeline,
    compute_group: wgpu::BindGroup,
    render_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    capacity: u32,
    envelopes: [[(f32, f32); 3]; 3],
    generated: Option<(u32, u32, i32, u32)>,
}

/// Initialize the shared resources. `target_format` must match egui's color target.
/// The render pipeline assumes egui's default single-sampled render pass.
pub fn setup(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    target_format: wgpu::TextureFormat,
    resources: &mut CallbackResources,
) {
    let limits = device.limits();
    let capacity = (limits.max_storage_buffer_binding_size as u64 / 32)
        .min(limits.max_buffer_size / 32)
        .min(u64::from(limits.max_compute_workgroups_per_dimension) * 64)
        .min(1_000_000) as u32;
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("orbital uniforms"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let particles = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("orbital particles"),
        size: u64::from(capacity.max(1)) * 32,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let make_layout = |read_only, visibility| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("orbital bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    };
    let compute_layout = make_layout(false, wgpu::ShaderStages::COMPUTE);
    let render_layout = make_layout(
        true,
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
    );
    let make_group = |layout: &wgpu::BindGroupLayout| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("orbital bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particles.as_entire_binding(),
                },
            ],
        })
    };
    let compute_group = make_group(&compute_layout);
    let render_group = make_group(&render_layout);
    let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("orbital sampling"),
        source: wgpu::ShaderSource::Wgsl(include_str!("orbital_compute.wgsl").into()),
    });
    let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("orbital sprites"),
        source: wgpu::ShaderSource::Wgsl(include_str!("orbital_render.wgsl").into()),
    });
    let cp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("orbital compute pipeline layout"),
        bind_group_layouts: &[&compute_layout],
        push_constant_ranges: &[],
    });
    let rp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("orbital render pipeline layout"),
        bind_group_layouts: &[&render_layout],
        push_constant_ranges: &[],
    });
    let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("orbital compute"),
        layout: Some(&cp_layout),
        module: &compute_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("orbital render"),
        layout: Some(&rp_layout),
        vertex: wgpu::VertexState {
            module: &render_shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &render_shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let mut envelopes = [[(0.0, 0.0); 3]; 3];
    for n in 1..=3 {
        for l in 0..n {
            envelopes[(n - 1) as usize][l as usize] = radial_envelope(n, l);
        }
    }
    resources.insert(Gpu {
        compute,
        render,
        compute_group,
        render_group,
        uniform,
        capacity,
        envelopes,
        generated: None,
    });
}

/// Create an egui paint callback. Dimensions and camera angles are in egui points
/// and radians respectively. `zoom` is a positive camera magnification.
pub fn callback(rect: egui::Rect, params: RenderParams) -> egui::PaintCallback {
    Callback::new_paint_callback(rect, Cloud { rect, params })
}
struct Cloud {
    rect: egui::Rect,
    params: RenderParams,
}

fn finite(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        default
    }
}

// Numerically bound r² R² for each of the six radial functions. A generous
// safety margin ensures the rejection envelope remains above grid maxima.
fn radial_envelope(n: u32, l: u32) -> (f32, f32) {
    let mut peak: f32 = 0.0;
    let mut density_peak: f32 = 0.0;
    for i in 0..=4000 {
        let r = i as f32 * 0.01;
        let f = match (n, l) {
            (1, _) => (-r).exp(),
            (2, 0) => (2.0 - r) * (-r / 2.0).exp(),
            (2, _) => r * (-r / 2.0).exp(),
            (3, 0) => (27.0 - 18.0 * r + 2.0 * r * r) * (-r / 3.0).exp(),
            (3, 1) => r * (6.0 - r) * (-r / 3.0).exp(),
            _ => r * r * (-r / 3.0).exp(),
        };
        peak = peak.max(r * r * f * f);
        density_peak = density_peak.max(f * f);
    }
    (peak * 1.01, density_peak * 1.01)
}

impl CallbackTrait for Cloud {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<Gpu>() else {
            return vec![];
        };
        // Samples are deterministic and stationary; time is reserved for future animation.
        let _time = self.params.time;
        let n = self.params.n.clamp(1, 3);
        let l = self.params.l.min(n - 1);
        let m = self.params.m.clamp(-(l as i32), l as i32);
        let count = self.params.count.min(gpu.capacity);
        let width = (self.rect.width() * screen.pixels_per_point).max(1.0);
        let height = (self.rect.height() * screen.pixels_per_point).max(1.0);
        let (envelope, density_peak) = gpu.envelopes[(n - 1) as usize][l as usize];
        let vals = [
            n,
            l,
            (m + 2) as u32,
            count,
            finite(self.params.yaw, 0.0).to_bits(),
            finite(self.params.pitch, 0.0).to_bits(),
            finite(self.params.zoom, 1.0).clamp(0.05, 1.5).to_bits(),
            (width / height).to_bits(),
            finite(self.params.point_size, 1.0)
                .clamp(0.0, 32.0)
                .to_bits(),
            finite(self.params.brightness, 1.0)
                .clamp(0.0, 20.0)
                .to_bits(),
            finite(self.params.threshold, 0.0).clamp(0.0, 1.0).to_bits(),
            (self.params.color_mode.min(2) as f32).to_bits(),
            envelope.to_bits(),
            density_peak.to_bits(),
            width.to_bits(),
            height.to_bits(),
        ];
        let mut bytes = [0u8; 64];
        for (chunk, val) in bytes.chunks_exact_mut(4).zip(vals) {
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        queue.write_buffer(&gpu.uniform, 0, &bytes);
        let key = (n, l, m, count);
        if gpu.generated != Some(key) && count > 0 {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hydrogen rejection sampling"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&gpu.compute);
            pass.set_bind_group(0, &gpu.compute_group, &[]);
            pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
            gpu.generated = Some(key);
        }
        vec![]
    }
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(gpu) = resources.get::<Gpu>() {
            pass.set_pipeline(&gpu.render);
            pass.set_bind_group(0, &gpu.render_group, &[]);
            pass.draw(0..6, 0..self.params.count.min(gpu.capacity));
        }
    }
}
