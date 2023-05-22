use bytemuck::Pod;
use iced_native::futures;
use std::marker::PhantomData;
use wgpu::util::DeviceExt;

use crate::fractal_view::View;

#[macro_export]
macro_rules! wgsl_shader_test {
    ($shader_file:expr, $($shader_content:tt)*) => {
        wgpu::ShaderModuleDescriptor {
            label: Some(concat!($shader_file, " (test)")),
            source: wgpu::ShaderSource::Wgsl(concat!(
                include_str!($shader_file),
                $($shader_content)*
            ).into()),
        }
    };
}

pub trait DescribableStruct {
    fn layout_entry() -> wgpu::BindGroupLayoutEntry;

    fn descriptor() -> wgpu::BufferDescriptor<'static>;
}

pub struct TransferrableBuffer<T: DescribableStruct + Pod> {
    staging_buffer: wgpu::Buffer,
    storage_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    phantom: PhantomData<T>,
}

impl<T: DescribableStruct + Pod> TransferrableBuffer<T> {
    pub fn new(device: &wgpu::Device, input: &T) -> Self {
        let staging_buffer = device.create_buffer(&T::descriptor());
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(input),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[T::layout_entry()],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }],
        });
        Self {
            staging_buffer,
            storage_buffer,
            bind_group_layout,
            bind_group,
            phantom: Default::default(),
        }
    }

    fn copy(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_buffer_to_buffer(
            &self.storage_buffer,
            0,
            &self.staging_buffer,
            0,
            std::mem::size_of::<T>() as u64,
        );
    }

    pub fn fetch_result(&self, device: &wgpu::Device) -> T {
        let buffer_slice = self.staging_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(wgpu::Maintain::Wait);
        futures::executor::block_on(receiver.receive());
        let data = buffer_slice.get_mapped_range();
        let result = *bytemuck::from_bytes::<T>(&data);
        drop(data);
        self.staging_buffer.unmap();
        result
    }
}

pub(crate) fn run_compute_shader<T: DescribableStruct + Pod>(
    view: &View,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &TransferrableBuffer<T>,
    shader_test_descriptor: wgpu::ShaderModuleDescriptor,
    entry_point: &'static str,
) {
    let module = device.create_shader_module(shader_test_descriptor);
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&view.pipeline_layout),
        module: &module,
        entry_point,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut compute_pass = encoder.begin_compute_pass(&Default::default());
        compute_pass.set_bind_group(0, &view.bind_group, &[]);
        compute_pass.set_bind_group(1, &buffer.bind_group, &[]);
        compute_pass.set_pipeline(&pipeline);
        compute_pass.dispatch_workgroups(1, 1, 1);
    }
    buffer.copy(&mut encoder);
    queue.submit(Some(encoder.finish()));
}
