mod controls;
mod fractal_view;
mod gpu;
#[cfg(test)]
#[macro_use]
mod wgpu_test;

use cgmath::Vector2;
use controls::{CanvasMessage, Controls, Message};
use fractal_view::View;
use gpu::Gpu;
use iced::Color;
use iced_wgpu::{wgpu, Backend, Renderer, Settings, Viewport};
use iced_winit::{
    conversion, program, renderer,
    winit::{self},
    Clipboard, Debug, Size,
};
use winit::{
    dpi::PhysicalPosition,
    event::{Event, ModifiersState, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowBuilderExtWebSys;

const ZOOM_SCROLL_FACTOR: f32 = 40.0;

pub fn main() {
    init_logging();
    let event_loop = EventLoop::new();
    let window = create_window(&event_loop);

    let physical_size = window.inner_size();
    let mut viewport = Viewport::with_physical_size(
        Size::new(physical_size.width, physical_size.height),
        window.scale_factor(),
    );
    let mut cursor_position = PhysicalPosition::new(-1.0, -1.0);
    let mut modifiers = ModifiersState::default();
    let mut clipboard = Clipboard::connect(&window);

    let (gpu, surface) = Gpu::new(&window);

    let mut resized = false;

    // Initialize staging belt
    let mut staging_belt = wgpu::util::StagingBelt::new(5 * 1024);

    // Initialize scene and GUI controls
    let mut fractal_view = View::new(&gpu, &[]);
    let controls = Controls::new();

    // Initialize iced
    let mut debug = Debug::new();
    let mut renderer = Renderer::new(Backend::new(
        &gpu.device,
        Settings::default(),
        gpu.texture_format,
    ));

    let mut state =
        program::State::new(controls, viewport.logical_size(), &mut renderer, &mut debug);

    // Run event loop
    event_loop.run(move |event, _, control_flow| {
        // You should change this if you want to render continuosly
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CursorMoved { position, .. } => {
                        cursor_position = position;
                    }
                    WindowEvent::ModifiersChanged(new_modifiers) => {
                        modifiers = new_modifiers;
                    }
                    WindowEvent::Resized(_) => {
                        resized = true;
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                }

                // Map window event to iced event
                if let Some(event) =
                    iced_winit::conversion::window_event(&event, window.scale_factor(), modifiers)
                {
                    state.queue_event(event);
                }
            }
            Event::MainEventsCleared => {
                // If there are events pending
                if !state.is_queue_empty() {
                    // We update iced
                    let _ = state.update(
                        viewport.logical_size(),
                        conversion::cursor_position(cursor_position, viewport.scale_factor()),
                        &mut renderer,
                        &iced_wgpu::Theme::Dark,
                        &renderer::Style {
                            text_color: Color::WHITE,
                        },
                        &mut clipboard,
                        &mut debug,
                    );

                    let program = state.program();

                    match program.take_last_message() {
                        Some(Message::Canvas(CanvasMessage::Pan(x, y))) => {
                            let displacement = Vector2::new(
                                x / physical_size.width as f32,
                                y / physical_size.height as f32,
                            );
                            fractal_view.translate(displacement);
                            state.queue_message(Message::Canvas(
                                CanvasMessage::UpdateViewTransform(
                                    fractal_view.get_view_transform(),
                                ),
                            ));
                        }
                        Some(Message::Canvas(CanvasMessage::Zoom(y, on_point))) => {
                            let factor = y / ZOOM_SCROLL_FACTOR + 1.0;
                            fractal_view.zoom(
                                factor,
                                Vector2::new(
                                    on_point.x / physical_size.width as f32 - 0.5,
                                    -on_point.y / physical_size.height as f32 + 0.5,
                                ),
                            );
                            state.queue_message(Message::Canvas(
                                CanvasMessage::UpdateViewTransform(
                                    fractal_view.get_view_transform(),
                                ),
                            ));
                        }
                        Some(Message::FractalTypeSelected(fractal_type)) => {
                            fractal_view.set_fractal_type(&gpu, fractal_type);
                        }
                        _ => {}
                    }

                    // and request a redraw
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                if resized {
                    let size = window.inner_size();

                    viewport = Viewport::with_physical_size(
                        Size::new(size.width, size.height),
                        window.scale_factor(),
                    );

                    gpu.configure_surface(&surface, size);

                    resized = false;
                }

                match surface.get_current_texture() {
                    Ok(frame) => {
                        fractal_view.update_transform(&gpu.queue);

                        let mut encoder =
                            gpu.device
                                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                    label: None,
                                });

                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        fractal_view.render(&view, &mut encoder);

                        // And then iced on top
                        renderer.with_primitives(|backend, primitive| {
                            backend.present(
                                &gpu.device,
                                &mut staging_belt,
                                &mut encoder,
                                &view,
                                primitive,
                                &viewport,
                                &debug.overlay(),
                            );
                        });

                        // Then we submit the work
                        staging_belt.finish();
                        gpu.queue.submit(Some(encoder.finish()));
                        frame.present();

                        // Update the mouse cursor
                        window.set_cursor_icon(iced_winit::conversion::mouse_interaction(
                            state.mouse_interaction(),
                        ));

                        // And recall staging buffers
                        staging_belt.recall();
                    }
                    Err(error) => match error {
                        wgpu::SurfaceError::OutOfMemory => {
                            panic!("Swapchain error: {error}. Rendering cannot continue.")
                        }
                        _ => {
                            // Try rendering again next frame.
                            window.request_redraw();
                        }
                    },
                }
            }
            _ => {}
        }
    })
}

#[cfg(target_arch = "wasm32")]
fn init_logging() {
    console_log::init_with_level(log::Level::Debug).expect("could not initialize logger");
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[cfg(not(target_arch = "wasm32"))]
fn init_logging() {
    env_logger::init();
}

#[cfg(target_arch = "wasm32")]
fn create_window(event_loop: &EventLoop<()>) -> iced_winit::winit::window::Window {
    let canvas_element = {
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.get_element_by_id("iced_canvas"))
            .and_then(|element| element.dyn_into::<HtmlCanvasElement>().ok())
            .expect("Canvas with id `iced_canvas` is missing")
    };
    winit::window::WindowBuilder::new()
        .with_canvas(Some(canvas_element))
        .build(event_loop)
        .expect("Failed to build winit window")
}

#[cfg(not(target_arch = "wasm32"))]
fn create_window(event_loop: &EventLoop<()>) -> iced_winit::winit::window::Window {
    winit::window::Window::new(event_loop).unwrap()
}
