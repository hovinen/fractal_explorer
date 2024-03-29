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
use iced_core::mouse::Cursor;
use iced_wgpu::{graphics::Viewport, wgpu, Backend, Renderer, Settings};
use iced_winit::{
    conversion,
    core::{renderer, Size},
    runtime::{program, Debug},
    winit::{self},
    Clipboard,
};
use winit::{
    dpi::PhysicalPosition,
    event::{Event, WindowEvent},
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
    let event_loop = EventLoop::new().unwrap();
    let window = create_window(&event_loop);

    let physical_size = window.inner_size();
    let mut viewport = Viewport::with_physical_size(
        Size::new(physical_size.width, physical_size.height),
        window.scale_factor(),
    );
    let mut cursor_position = PhysicalPosition::new(-1.0, -1.0);
    let mut clipboard = Clipboard::connect(&window);

    let (gpu, surface) = Gpu::new(&window);

    let mut modifiers = winit::keyboard::ModifiersState::default();

    let mut resized = false;

    // Initialize staging belt
    let mut staging_belt = wgpu::util::StagingBelt::new(5 * 1024);

    // Initialize scene and GUI controls
    let mut fractal_view = View::new(&gpu);
    let controls = Controls::new();

    // Initialize iced
    let mut debug = Debug::new();
    let renderer = Renderer::new(
        Backend::new(
            &gpu.device,
            &gpu.queue,
            Settings::default(),
            gpu.texture_format,
        ),
        iced::Font::DEFAULT,
        iced::Pixels::from(14.0),
    );
    let mut widget_renderer = iced_widget::renderer::Renderer::Wgpu(renderer);

    let mut state = program::State::new(
        controls,
        viewport.logical_size(),
        &mut widget_renderer,
        &mut debug,
    );

    // Run event loop
    event_loop
        .run(|event, event_loop_window| {
            // You should change this if you want to render continuosly
            event_loop_window.set_control_flow(ControlFlow::Wait);

            match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::CursorMoved { position, .. } => {
                            cursor_position = position;
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            modifiers = new_modifiers.state();
                        }
                        WindowEvent::Resized(_) => {
                            resized = true;
                        }
                        WindowEvent::CloseRequested => {
                            event_loop_window.exit();
                        }
                        WindowEvent::RedrawRequested => {
                            redraw(
                                &window,
                                &surface,
                                &gpu,
                                &mut fractal_view,
                                &mut widget_renderer,
                                &mut state,
                                &mut viewport,
                                &mut staging_belt,
                                &mut debug,
                                &mut resized,
                            );
                        }
                        _ => {}
                    }

                    // Map window event to iced event
                    if let Some(event) = iced_winit::conversion::window_event(
                        iced_core::window::Id::MAIN,
                        event,
                        window.scale_factor(),
                        modifiers,
                    ) {
                        state.queue_event(event);
                    }
                }
                Event::AboutToWait => {
                    // If there are events pending
                    if !state.is_queue_empty() {
                        // We update iced
                        let _ = state.update(
                            viewport.logical_size(),
                            Cursor::Available(conversion::cursor_position(
                                cursor_position,
                                viewport.scale_factor(),
                            )),
                            &mut widget_renderer,
                            &iced_winit::style::Theme::Dark,
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
                _ => {}
            }
        })
        .unwrap();
}

fn redraw(
    window: &winit::window::Window,
    surface: &wgpu::Surface,
    gpu: &Gpu,
    fractal_view: &mut View,
    widget_renderer: &mut iced_widget::renderer::Renderer,
    state: &mut program::State<Controls>,
    viewport: &mut Viewport,
    staging_belt: &mut wgpu::util::StagingBelt,
    debug: &mut Debug,
    resized: &mut bool,
) {
    if *resized {
        let size = window.inner_size();

        *viewport =
            Viewport::with_physical_size(Size::new(size.width, size.height), window.scale_factor());

        gpu.configure_surface(&surface, size);

        *resized = false;
    }

    match surface.get_current_texture() {
        Ok(frame) => {
            fractal_view.update_transform(&gpu.queue);

            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            fractal_view.render(&view, &mut encoder);

            // And then iced on top
            let iced_widget::renderer::Renderer::Wgpu(renderer) = widget_renderer else {
                panic!("Not the right kind of renderer!")
            };
            renderer.with_primitives(|backend, primitive| {
                backend.present(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    None,
                    gpu.texture_format,
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
