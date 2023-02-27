use iced::widget::Row;
use iced_winit::Program;

pub(super) struct Controls {}

#[derive(Debug)]
pub(super) enum Message {}

impl Controls {
    pub(super) fn new() -> Self {
        Self {}
    }
}

impl Program for Controls {
    type Renderer = iced_wgpu::Renderer;
    type Message = Message;

    fn update(&mut self, _message: Self::Message) -> iced::Command<Self::Message> {
        iced::Command::none()
    }

    fn view(&self) -> iced_winit::Element<'_, Self::Message, Self::Renderer> {
        Row::new().into()
    }
}
