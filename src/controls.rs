use std::{cell::Cell, fmt::Display};

use iced::{
    mouse::{self, Button, ScrollDelta},
    widget::{pick_list, Row},
    Element, Length, Rectangle,
};
use iced_graphics::widget::{
    canvas::{self, event::Status, Cursor, Event, Geometry},
    Canvas,
};
use iced_native::Theme;
use iced_winit::Program;

pub(super) struct Controls {
    canvas: FractalCanvas,
    current_type: FractalType,
    last_message: Cell<Option<Message>>,
}

#[derive(Debug)]
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
    type Renderer = iced_wgpu::Renderer;
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Message::Canvas(_) => {}
            Message::FractalTypeSelected(selected_type) => {
                self.current_type = selected_type;
            }
        }
        self.last_message.set(Some(message));
        iced::Command::none()
    }

    fn view(&self) -> iced_winit::Element<'_, Self::Message, Self::Renderer> {
        Row::new()
            .push(
                self.canvas
                    .view()
                    .map(move |message| Message::Canvas(message)),
            )
            .push(pick_list(
                &FractalType::ALL[..],
                Some(self.current_type),
                |t| Message::FractalTypeSelected(t),
            ))
            .into()
    }
}

struct FractalCanvas {}

#[derive(Debug)]
pub(super) enum CanvasMessage {
    Pan(f32, f32),
    Zoom(f32),
}

#[derive(Default)]
enum Mode {
    #[default]
    None,
    Panning {
        start_position: iced::Point,
    },
}

impl FractalCanvas {
    fn new() -> Self {
        Self {}
    }

    fn view(&self) -> Element<CanvasMessage> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl canvas::Program<CanvasMessage> for FractalCanvas {
    type State = Mode;

    fn draw(
        &self,
        _state: &Self::State,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        vec![]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: Event,
        _bounds: Rectangle,
        cursor: Cursor,
    ) -> (Status, Option<CanvasMessage>) {
        match event {
            Event::Mouse(event) => match event {
                mouse::Event::CursorEntered => (Status::Ignored, None),
                mouse::Event::CursorLeft => (Status::Ignored, None),
                mouse::Event::CursorMoved { position } => {
                    let (result, new_mode) = match state {
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
                    *state = new_mode;
                    result
                }
                mouse::Event::ButtonPressed(button) => {
                    if button == Button::Left {
                        if let Some(position) = cursor.position() {
                            *state = Mode::Panning {
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
                        *state = Mode::None;
                        (Status::Captured, None)
                    } else {
                        (Status::Ignored, None)
                    }
                }
                mouse::Event::WheelScrolled {
                    // TODO: Zoom in on the location of the cursor rather than just into the centre.
                    delta: ScrollDelta::Pixels { x: _x, y },
                } => (Status::Captured, Some(CanvasMessage::Zoom(y))),
                mouse::Event::WheelScrolled {
                    delta: ScrollDelta::Lines { x: _x, y },
                } => (Status::Captured, Some(CanvasMessage::Zoom(y))),
            },
            Event::Touch(_) => (Status::Ignored, None),
            Event::Keyboard(_) => (Status::Ignored, None),
        }
    }
}
