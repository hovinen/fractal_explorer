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
        let instance = wgpu::Instance::new(backend);
        let surface = Self::create_surface(&instance, window);
        let (device, queue, texture_format) =
            Self::create_device(&instance, Some(&surface), backend);
        (
            Self {
                texture_format,
                device,
                queue,
            },
            surface,
        )
    }

    #[cfg(test)]
    pub fn new_without_surface() -> Self {
        let backend = Self::get_backend();
        let instance = wgpu::Instance::new(backend);
        let (device, queue, texture_format) = Self::create_device(&instance, None, backend);
        Self {
            texture_format,
            device,
            queue,
        }
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
        let (texture_format, (device, queue)) = futures::executor::block_on(async {
            let adapter =
                wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, surface)
                    .await
                    .expect("No suitable GPU adapters found on the system!");

            let adapter_features = adapter.features();

            #[cfg(target_arch = "wasm32")]
            let needed_limits =
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());

            #[cfg(not(target_arch = "wasm32"))]
            let needed_limits = wgpu::Limits::default();

            (
                surface
                    .map(|s| {
                        s.get_supported_formats(&adapter)
                            .first()
                            .copied()
                            .expect("Get preferred format")
                    })
                    .unwrap_or(wgpu::TextureFormat::Rgba8Unorm),
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
            )
        });
        (device, queue, texture_format)
    }

    fn create_surface(instance: &wgpu::Instance, window: &winit::window::Window) -> wgpu::Surface {
        unsafe { instance.create_surface(&window) }
    }
}
