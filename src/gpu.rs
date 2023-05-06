use iced::futures;
use iced_winit::winit;

pub struct Gpu {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) texture_format: wgpu::TextureFormat,
}

impl Gpu {
    pub fn new(window: &winit::window::Window) -> (Self, wgpu::Surface) {
        let backend = Self::get_backend();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..Default::default()
        });
        let surface = unsafe { instance.create_surface(&window).unwrap() };
        let (device, queue, texture_format) =
            Self::create_device(&instance, Some(&surface), backend);
        let gpu = Self {
            texture_format,
            device,
            queue,
        };
        let physical_size = window.inner_size();
        gpu.configure_surface(&surface, physical_size);
        (gpu, surface)
    }

    #[cfg(test)]
    pub fn new_without_surface() -> Self {
        let backend = Self::get_backend();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..Default::default()
        });
        let (device, queue, texture_format) = Self::create_device(&instance, None, backend);
        Self {
            texture_format,
            device,
            queue,
        }
    }

    pub fn configure_surface(&self, surface: &wgpu::Surface, size: winit::dpi::PhysicalSize<u32>) {
        surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.texture_format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        );
    }

    fn get_backend() -> wgpu::Backends {
        let default_backend = if cfg!(target_arch = "wasm32") {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::PRIMARY
        };
        wgpu::util::backend_bits_from_env().unwrap_or(default_backend)
    }

    fn create_device(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface>,
        backends: wgpu::Backends,
    ) -> (wgpu::Device, wgpu::Queue, wgpu::TextureFormat) {
        let ((device, queue), texture_format) = futures::executor::block_on(async {
            let adapter =
                wgpu::util::initialize_adapter_from_env_or_default(instance, backends, surface)
                    .await
                    .expect("No suitable GPU adapters found on the system!");

            let adapter_features = adapter.features();

            let needed_limits = if cfg!(target_arch = "wasm32") {
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
            } else {
                wgpu::Limits::default()
            };

            (
                adapter
                    .request_device(
                        &wgpu::DeviceDescriptor {
                            label: None,
                            features: adapter_features & wgpu::Features::default(),
                            limits: needed_limits,
                        },
                        None,
                    )
                    .await
                    .expect("Request device"),
                surface
                    .map(|s| {
                        s.get_capabilities(&adapter)
                            .formats
                            .first()
                            .copied()
                            .expect("Get preferred format")
                    })
                    .unwrap_or(wgpu::TextureFormat::Rgba8Unorm),
            )
        });
        (device, queue, texture_format)
    }
}
