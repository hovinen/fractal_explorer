use crate::{controls::FractalType, gpu::Gpu};
use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix, Matrix3, Vector2};
use iced_wgpu::wgpu::{self, util::DeviceExt};
use std::num::NonZeroU64;

// Two triangles which form a square [-1,-1] - [1,1]
const VERTICES: &[[f32; 2]] = &[[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];
const INDICES: &[[u16; 3]] = &[[0, 1, 2], [1, 2, 3]];

const ORIGINAL_VIEWPORT_WIDTH: f32 = 4.0;

pub(super) struct View {
    pipeline_layout: wgpu::PipelineLayout,
    fs_module: wgpu::ShaderModule,
    vs_module: wgpu::ShaderModule,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    #[cfg(test)]
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    view_transform: Matrix3<f32>,
}

impl View {
    pub(super) fn new(gpu: &Gpu) -> Self {
        let vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });
        let view_transform =
            Matrix3::from_scale(2.0) * Matrix3::from_translation(Vector2::new(-0.25, 0.0));
        let uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform buffer"),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            size: std::mem::size_of::<Uniform>() as u64,
            mapped_at_creation: false,
        });
        let bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Bind group layout"),
                    entries: &[Uniform::layout_entry()],
                });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                push_constant_ranges: &[],
                bind_group_layouts: &[&bind_group_layout],
            });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let (vs_module, fs_module) = (
            gpu.device
                .create_shader_module(wgpu::include_wgsl!("shader/vert.wgsl")),
            gpu.device
                .create_shader_module(wgpu::include_wgsl!("shader/frag.wgsl")),
        );
        let pipeline = Self::build_pipeline(
            gpu,
            &pipeline_layout,
            &vs_module,
            &fs_module,
            Self::entry_point_for_fractal_type(FractalType::Mandelbrot),
        );
        Self {
            pipeline_layout,
            fs_module,
            vs_module,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            view_transform,
            bind_group,
            #[cfg(test)]
            bind_group_layout,
        }
    }

    pub(super) fn render(&self, target: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }

    pub(super) fn update_transform(&self, queue: &iced_wgpu::wgpu::Queue) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice::<Uniform, _>(&[self.view_transform.into()]),
        );
    }

    pub(super) fn translate(&mut self, displacement: Vector2<f32>) {
        self.view_transform =
            self.view_transform * Matrix3::from_translation(ORIGINAL_VIEWPORT_WIDTH * displacement);
    }

    pub(super) fn zoom(&mut self, factor: f32, on_point: Vector2<f32>) {
        self.view_transform = self.view_transform
            * Matrix3::from_translation(ORIGINAL_VIEWPORT_WIDTH / 2.0 * on_point)
            * Matrix3::from_scale(factor)
            * Matrix3::from_translation(-ORIGINAL_VIEWPORT_WIDTH / 2.0 * on_point);
    }

    pub(super) fn get_view_transform(&self) -> Matrix3<f32> {
        self.view_transform
    }

    pub(super) fn set_fractal_type(&mut self, gpu: &Gpu, fractal_type: FractalType) {
        self.pipeline = Self::build_pipeline(
            gpu,
            &self.pipeline_layout,
            &self.vs_module,
            &self.fs_module,
            Self::entry_point_for_fractal_type(fractal_type),
        );
    }

    fn build_pipeline(
        gpu: &Gpu,
        pipeline_layout: &wgpu::PipelineLayout,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
        entry_point: &'static str,
    ) -> wgpu::RenderPipeline {
        gpu.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: vs_module,
                    entry_point: "main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: fs_module,
                    entry_point,
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.texture_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            })
    }

    fn entry_point_for_fractal_type(fractal_type: FractalType) -> &'static str {
        match fractal_type {
            FractalType::Mandelbrot => "mandelbrot",
            FractalType::Newton => "newton",
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
struct Uniform {
    transform_1: [f32; 3],
    _padding_1: f32,
    transform_2: [f32; 3],
    _padding_2: f32,
    transform_3: [f32; 3],
    _padding_3: f32,
}

impl Uniform {
    pub fn layout_entry() -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(std::mem::size_of::<Self>() as u64),
            },
            count: None,
        }
    }
}

impl From<Matrix3<f32>> for Uniform {
    fn from(value: Matrix3<f32>) -> Self {
        let value = value.transpose(); // Input is column-major
        Self {
            transform_1: value.row(0).into(),
            transform_2: value.row(1).into(),
            transform_3: value.row(2).into(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::View;
    use crate::{
        gpu::Gpu,
        wgpu_test::{DescribableStruct, GpuTestHarness},
        wgsl_shader_test,
    };
    use bytemuck::{Pod, Zeroable};
    use cgmath::Vector3;
    use googletest::matchers::__internal_unstable_do_not_depend_on_these::ElementsAre;
    use googletest::prelude::*;

    #[async_std::test]
    async fn transform_is_transferred_correctly() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(1.0, 2.0, 1.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn apply_uniform() {
                    let v_out = u.transform * v;
                    v = v_out;
                }
            "
        );

        harness.run_compute_shader(test_shader, "apply_uniform");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            eq(MappableVector([1.5, 4.0, 1.0]))
        )
    }

    #[async_std::test]
    async fn mandelbrot_iteration_is_applied_correctly_inside_set() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(-0.5, 0.5, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_mandelbrot_iteration() {
                    let i = mandelbrot_iterations(vec2(v.x, v.y));
                    v = vec3(i, 0.0, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_mandelbrot_iteration");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            eq(MappableVector([0.0, 0.0, 0.0]))
        )
    }

    #[async_std::test]
    async fn mandelbrot_iteration_is_applied_correctly_outside_set() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(0.5, 0.6, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_mandelbrot_iteration() {
                    let i = mandelbrot_iterations(vec2(v.x, v.y));
                    v = vec3(i, 0.0, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_mandelbrot_iteration");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            // TODO: Using the elements_are! macro directly causes the Rust
            // compiler to be utterly confused with type inference, inferring
            // the container type to be an infinitely recursive type coming from
            // the palette crate for some reason. So we have to use the actual
            // struct and add an explicit type argument. Figure out why this is
            // happening and solve the root cause, if possible.
            matches_pattern!(MappableVector(ElementsAre::<[f32; 3], _>::new(vec![
                Box::new(gt(0.0)),
                Box::new(eq(0.0)),
                Box::new(eq(0.0)),
            ])))
        )
    }

    #[async_std::test]
    async fn eval_poly_evaluates_correctly_at_root() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(-0.5, 3.0f32.sqrt() / 2.0, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_eval_poly() {
                    let result = eval_poly(vec2(v.x, v.y), COEFFS);
                    v = vec3(result, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_eval_poly");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            matches_pattern!(MappableVector(ElementsAre::<[f32; 3], _>::new(vec![
                Box::new(approx_eq(0.0)),
                Box::new(approx_eq(0.0)),
                Box::new(eq(0.0)),
            ])))
        )
    }

    #[async_std::test]
    async fn eval_poly_on_derivative_evaluates_correctly() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(2.0, 0.0, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_eval_poly_df() {
                    let result = eval_poly(vec2(v.x, v.y), DERIVATIVE_COEFFS);
                    v = vec3(result, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_eval_poly_df");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            matches_pattern!(MappableVector(ElementsAre::<[f32; 3], _>::new(vec![
                Box::new(approx_eq(12.0)),
                Box::new(approx_eq(0.0)),
                Box::new(eq(0.0)),
            ])))
        )
    }

    #[async_std::test]
    async fn inv_calculates_correct_inverse() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(-2.0, 1.5, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_inv() {
                    let result = inv(vec2(v.x, v.y));
                    v = vec3(result, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_inv");

        let inv = harness.fetch_result(&gpu.device).await;
        let inv_times_input = (
            inv.0[0] * input.0[0] - inv.0[1] * input.0[1],
            inv.0[0] * input.0[1] + inv.0[1] * input.0[0],
        );
        verify_that!(inv_times_input, (approx_eq(1.0), approx_eq(0.0)))
    }

    #[async_std::test]
    async fn newton_converges_to_root() -> Result<()> {
        let gpu = Gpu::new_without_surface();
        let input = MappableVector(Vector3::new(-2.0, 5.0, 0.0).into());
        let view = create_view(&gpu);
        let harness = GpuTestHarness::new(&gpu.device, &gpu.queue, &input).with_bind_group(
            0,
            &view.bind_group,
            &view.bind_group_layout,
        );
        let test_shader = wgsl_shader_test!(
            "shader/frag.wgsl",
            "
                @group(1) @binding(0) var<storage, read_write> v: vec3<f32>;

                @compute
                @workgroup_size(1)
                fn run_newton() {
                    let result = newton_iterate(v);
                    v = vec3(result, 0.0);
                }
            "
        );

        harness.run_compute_shader(test_shader, "run_newton");

        verify_that!(
            harness.fetch_result(&gpu.device).await,
            matches_pattern!(MappableVector(ElementsAre::<[f32; 3], _>::new(vec![
                Box::new(approx_eq(-0.5)),
                Box::new(approx_eq(3.0f32.sqrt() / 2.0)),
                Box::new(eq(0.0)),
            ])))
        )
    }

    fn create_view(gpu: &Gpu) -> View {
        let view = View::new(&gpu);
        view.update_transform(&gpu.queue);
        view
    }

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
    struct MappableVector([f32; 3]);

    impl DescribableStruct for MappableVector {}
}
