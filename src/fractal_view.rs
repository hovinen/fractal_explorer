use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix, Matrix3, Vector2};
use iced_wgpu::wgpu::{self, util::DeviceExt};
use wgpu::BindGroupLayout;

// Two triangles which form a square [-1,-1] - [1,1]
const VERTICES: &[[f32; 2]] = &[[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];
const INDICES: &[[u16; 3]] = &[[0, 1, 2], [1, 2, 3]];

const ORIGINAL_VIEWPORT_WIDTH: f32 = 4.0;

pub(super) struct View {
    texture_format: wgpu::TextureFormat,
    pipeline_layout: wgpu::PipelineLayout,
    fs_module: wgpu::ShaderModule,
    vs_module: wgpu::ShaderModule,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    view_transform: Matrix3<f32>,
}

impl View {
    pub(super) fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        extra_bind_group_layouts: &[&BindGroupLayout],
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let view_transform =
            Matrix3::from_scale(2.0) * Matrix3::from_translation(Vector2::new(-0.25, 0.0));
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform buffer"),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            size: std::mem::size_of::<Uniform>() as u64,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind group layout"),
            entries: &[Uniform::layout_entry()],
        });
        let mut bind_group_layouts = vec![&bind_group_layout];
        bind_group_layouts.extend(extra_bind_group_layouts);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            push_constant_ranges: &[],
            bind_group_layouts: bind_group_layouts.as_slice(),
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let (vs_module, fs_module) = (
            device.create_shader_module(wgpu::include_wgsl!("shader/vert.wgsl")),
            device.create_shader_module(wgpu::include_wgsl!("shader/frag.wgsl")),
        );
        let pipeline = Self::build_pipeline(
            device,
            texture_format,
            &pipeline_layout,
            &vs_module,
            &fs_module,
        );
        Self {
            texture_format,
            pipeline_layout,
            fs_module,
            vs_module,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            view_transform,
            bind_group,
        }
    }

    pub(super) fn clear<'a>(
        &self,
        target: &'a wgpu::TextureView,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        })
    }

    pub(super) fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
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

    pub(super) fn zoom(&mut self, factor: f32) {
        self.view_transform = self.view_transform * Matrix3::from_scale(factor);
    }

    fn build_pipeline(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        pipeline_layout: &wgpu::PipelineLayout,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
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
                entry_point: "mandelbrot",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
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
    use super::*;
    use crate::create_device;
    use cgmath::Vector3;
    use googletest::{matchers::eq, verify_that, Result};

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
    struct MappableVector([f32; 3]);

    impl MappableVector {
        fn layout_entry() -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<Self>() as u64),
                },
                count: None,
            }
        }
    }

    #[test]
    fn transform_is_transferred_correctly() -> Result<()> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
        let (format, device, queue) = create_device(&instance, None, wgpu::Backends::PRIMARY);

        let vec = MappableVector(Vector3::new(1.0, 2.0, 3.0).into());

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<MappableVector>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&vec),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[MappableVector::layout_entry()],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }],
        });
        let view = View::new(&device, format, &[&bind_group_layout]);

        view.update_transform(&queue);

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&view.pipeline_layout),
            module: &view.fs_module,
            entry_point: "fetch_uniform",
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut compute_pass = encoder.begin_compute_pass(&Default::default());
            compute_pass.set_bind_group(0, &view.bind_group, &[]);
            compute_pass.set_bind_group(1, &bind_group, &[]);
            compute_pass.set_pipeline(&pipeline);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &storage_buffer,
            0,
            &staging_buffer,
            0,
            std::mem::size_of::<MappableVector>() as u64,
        );
        queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(wgpu::Maintain::Wait);
        pollster::block_on(receiver.receive());
        let data = buffer_slice.get_mapped_range();
        let result = *bytemuck::from_bytes::<MappableVector>(&data);
        drop(data);
        staging_buffer.unmap();

        verify_that!(result, eq(MappableVector([0.5, 4.0, 3.0])))
    }
}
