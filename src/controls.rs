use cgmath::{Matrix3, Vector2, Vector3};
use iced::{
    mouse::{self, Button, Cursor, ScrollDelta},
    widget::{pick_list, Row},
    Color, Length, Point, Rectangle,
};
use iced_widget::{
    canvas::{self, event::Status, Event, Frame, Geometry, Text},
    Canvas,
};
use iced_winit::{core::Element, runtime::Program, style::Theme};
use std::{cell::Cell, fmt::Display};

pub(super) struct Controls {
    canvas: FractalCanvas,
    current_type: FractalType,
    last_message: Cell<Option<Message>>,
}

#[derive(Debug, Clone)]
pub(super) enum Message {
    Canvas(CanvasMessage),
    FractalTypeSelected(FractalType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FractalType {
    Mandelbrot,
    Newton,
}

impl FractalType {
    const ALL: [FractalType; 2] = [Self::Mandelbrot, Self::Newton];
}

impl Display for FractalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FractalType::Mandelbrot => write!(f, "Mandelbrot"),
            FractalType::Newton => write!(f, "Newton"),
        }
    }
}

impl Controls {
    pub(super) fn new() -> Self {
        Self {
            canvas: FractalCanvas::new(),
            current_type: FractalType::Mandelbrot,
            last_message: Cell::new(None),
        }
    }

    pub(super) fn take_last_message(&self) -> Option<Message> {
        self.last_message.take()
    }
}

impl Program for Controls {
    type Renderer = iced_widget::renderer::Renderer;
    type Message = Message;
    type Theme = Theme;

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Canvas(CanvasMessage::UpdateViewTransform(view_transform)) => {
                self.canvas.view_transform = view_transform;
            }
            Message::Canvas(_) => {}
            Message::FractalTypeSelected(selected_type) => {
                self.current_type = selected_type;
            }
        }
        self.last_message.set(Some(message));
        iced::Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message, Self::Theme, Self::Renderer> {
        Row::new()
            .push(self.canvas.view().map(Message::Canvas))
            .push(pick_list(
                &FractalType::ALL[..],
                Some(self.current_type),
                Message::FractalTypeSelected,
            ))
            .into()
    }
}

struct FractalCanvas {
    view_transform: Matrix3<f32>,
}

#[derive(Debug, Clone)]
pub(super) enum CanvasMessage {
    Pan(f32, f32),
    Zoom(f32, Point),
    UpdateViewTransform(Matrix3<f32>),
}

#[derive(Debug, Default)]
struct State {
    mode: Mode,
}

#[derive(Debug, Default)]
enum Mode {
    #[default]
    None,
    Panning {
        start_position: iced::Point,
    },
}

impl FractalCanvas {
    fn new() -> Self {
        Self {
            view_transform: Matrix3::from_scale(2.0)
                * Matrix3::from_translation(Vector2::new(-0.25, 0.0)),
        }
    }

    fn view(&self) -> Element<CanvasMessage, Theme, iced_widget::renderer::Renderer> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl canvas::Program<CanvasMessage, Theme, iced_widget::renderer::Renderer> for FractalCanvas {
    type State = State;

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced_widget::renderer::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Vec<Geometry> {
        if let Some(cursor_position) = cursor.position() {
            let transfromed_position = self.view_transform
                * Vector3::new(
                    cursor_position.x / bounds.width * 2.0 - 1.0,
                    cursor_position.y / bounds.height * 2.0 - 1.0,
                    1.0,
                );
            let mut position_text: Text = format!(
                "{:.4}+{:.4}i",
                transfromed_position.x, -transfromed_position.y
            )
            .into();
            position_text.color = Color::WHITE;
            let mut frame = Frame::new(renderer, bounds.size());
            frame.fill_text(position_text);
            vec![frame.into_geometry()]
        } else {
            vec![]
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> (Status, Option<CanvasMessage>) {
        match event {
            Event::Mouse(event) => match event {
                mouse::Event::CursorEntered => (Status::Ignored, None),
                mouse::Event::CursorLeft => (Status::Ignored, None),
                mouse::Event::CursorMoved { position } => {
                    let (result, new_mode) = match state.mode {
                        Mode::None => ((Status::Ignored, None), Mode::None),
                        Mode::Panning { start_position } => (
                            (
                                Status::Captured,
                                Some(CanvasMessage::Pan(
                                    start_position.x - position.x,
                                    position.y - start_position.y,
                                )),
                            ),
                            Mode::Panning {
                                start_position: position,
                            },
                        ),
                    };
                    state.mode = new_mode;
                    result
                }
                mouse::Event::ButtonPressed(button) => {
                    if button == Button::Left {
                        if let Some(position) = cursor.position() {
                            state.mode = Mode::Panning {
                                start_position: position,
                            };
                            (Status::Captured, None)
                        } else {
                            (Status::Ignored, None)
                        }
                    } else {
                        (Status::Ignored, None)
                    }
                }
                mouse::Event::ButtonReleased(button) => {
                    if button == Button::Left {
                        state.mode = Mode::None;
                        (Status::Captured, None)
                    } else {
                        (Status::Ignored, None)
                    }
                }
                mouse::Event::WheelScrolled {
                    // TODO: Zoom in on the location of the cursor rather than just into the centre.
                    delta: ScrollDelta::Pixels { x: _x, y },
                } => {
                    let on_point = cursor.position().unwrap_or(bounds.center());
                    (Status::Captured, Some(CanvasMessage::Zoom(y, on_point)))
                }
                mouse::Event::WheelScrolled {
                    delta: ScrollDelta::Lines { x: _x, y },
                } => {
                    let on_point = cursor.position().unwrap_or(bounds.center());
                    (Status::Captured, Some(CanvasMessage::Zoom(y, on_point)))
                }
            },
            Event::Touch(_) => (Status::Ignored, None),
            Event::Keyboard(_) => (Status::Ignored, None),
        }
    }
}
