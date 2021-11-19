use iced::{Application, Canvas, Column, Command, Element, Length, ProgressBar, Sandbox, Settings, Size, Slider, slider};
use iced_native::{Widget, Clipboard, Color, layout, renderer};

use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};


#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {

    #[structopt(long, default_value="info")]
    pub log_level: LevelFilter,
}


fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    App::run(Settings::default())?;

    Ok(())
}

struct App {
    value: f32,
    progress_bar_slider: slider::State,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Message {
    SliderChanged(f32),
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (Self{
            value: 0.0,
            progress_bar_slider: Default::default(),
            //cg: CurveGraph{s: 0.0, v: None}
        }, Command::none())
    }

    fn title(&self) -> String {
        "VMouse GUI".to_string()
    }

    fn update(
        &mut self,
        message: Self::Message
    ) -> iced::Command<Self::Message> {
        //TODO: update things
        match message {
            Message::SliderChanged(x) => self.value = x,
        }

        Command::none()
    }

    fn view(&mut self) -> iced::Element<'_, Self::Message> {
        Column::new()
            .padding(20)
            .push(ProgressBar::new(0.0..=100.0, self.value))
            .push(
                Slider::new(
                    &mut self.progress_bar_slider,
                    0.0..=100.0,
                    self.value,
                    Message::SliderChanged,
                )
                .step(0.01),
            )
            .push(
                CurveGraph{s: self.value, v: None}
            )
            .into()
    }
}

pub struct CurveGraph {
    pub s: f32,
    pub v: Option<f32>,
}

impl <M, R> Widget<M, R> for CurveGraph 
where
    R: renderer::Renderer,
{
    fn width(&self) -> Length {
        Length::Shrink
    }

    fn height(&self) -> Length {
        Length::Shrink
    }

    fn layout(
        &self,
        renderer: &R,
        limits: &iced_native::layout::Limits,
    ) -> iced_native::layout::Node {
        layout::Node::new(Size::new(200.0, 200.0))
    }

    fn draw(
        &self,
        renderer: &mut R,
        style: &renderer::Style,
        layout: iced_native::Layout<'_>,
        cursor_position: iced::Point,
        viewport: &iced::Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border_radius: 1.0,
                border_width: 1.0,
                border_color: Color::BLACK,
            },
            Color::WHITE,
        )
    }

    fn hash_layout(&self, state: &mut iced_native::Hasher) {
        use std::hash::Hash;

        let mut b = self.s.to_bits();
        if let Some(v) = self.v {
            b ^= v.to_bits();
        }

        b.hash(state)
    }
}

impl<'a, M> Into<Element<'a, M>> for CurveGraph{
    fn into(self) -> Element<'a, M> {
        Element::new(self)
    }
}
