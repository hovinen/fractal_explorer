use bytemuck::Pod;
use std::{marker::PhantomData, num::NonZeroU64};
use wgpu::util::DeviceExt;

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
    fn layout_entry() -> wgpu::BindGroupLayoutEntry
    where
        Self: Sized,
    {
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

    fn descriptor() -> wgpu::BufferDescriptor<'static>
    where
        Self: Sized,
    {
        wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Self>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }
    }
}

pub struct GpuTestHarness<'a, T: DescribableStruct + Pod> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    staging_buffer: wgpu::Buffer,
    storage_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    phantom: PhantomData<T>,
}

impl<'a, T: DescribableStruct + Pod> GpuTestHarness<'a, T> {
    pub fn new(device: &'a wgpu::Device, queue: &'a wgpu::Queue, input: &T) -> Self {
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
            device,
            queue,
            staging_buffer,
            storage_buffer,
            bind_group_layout,
            bind_group,
            phantom: Default::default(),
        }
    }

    pub async fn fetch_result(&self, device: &wgpu::Device) -> T {
        let buffer_slice = self.staging_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(wgpu::Maintain::Wait);
        receiver.receive().await;
        let data = buffer_slice.get_mapped_range();
        let result = *bytemuck::from_bytes::<T>(&data);
        drop(data);
        self.staging_buffer.unmap();
        result
    }

    pub fn run_compute_shader(
        &self,
        pipeline_layout: &wgpu::PipelineLayout,
        bind_group: &wgpu::BindGroup,
        shader_test_descriptor: wgpu::ShaderModuleDescriptor,
        entry_point: &'static str,
    ) {
        let module = self.device.create_shader_module(shader_test_descriptor);
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point,
            });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut compute_pass = encoder.begin_compute_pass(&Default::default());
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.set_bind_group(1, &self.bind_group, &[]);
            compute_pass.set_pipeline(&pipeline);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }
        self.copy(&mut encoder);
        self.queue.submit(Some(encoder.finish()));
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
}
