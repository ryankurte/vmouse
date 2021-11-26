use std::{ops::{Index, IndexMut}, time::Duration};

use futures::{StreamExt, stream::BoxStream};
use iced::{Application, Canvas, Column, Command, Container, Element, Length, PickList, Point, ProgressBar, Row, Sandbox, Settings, Size, Slider, Vector, canvas::{self, Cache, Frame, LineCap, Path, Stroke, Text}, pick_list, slider};
use iced_native::{Clipboard, Color, Widget, layout, renderer, subscription::Recipe};

use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};
use strum_macros::Display;


#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {

    #[structopt(long, default_value="info")]
    pub log_level: LevelFilter,
}


#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    App::run(Settings::default())?;

    Ok(())
}

struct App {
    values: AxisCollection<f32>,
    scales: AxisCollection<f32>,
    scale_slider: AxisCollection<slider::State>,
    value_slider: AxisCollection<slider::State>,
    cgs: AxisCollection<CurveGraph>,

    pick_axis: pick_list::State<Axis>,
    axis: Axis,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Message {
    ScaleChanged(Axis, f32),
    ValueChanged(Axis, f32),
    SelectAxis(Axis),
    Tick,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Display)]
pub enum Axis {
    X, Y, Z, RX, RY, RZ
}

pub const AXIS: &[Axis] = &[
    Axis::X, Axis::Y, Axis::Z, Axis::RX, Axis::RY, Axis::RZ
];

pub const AXIS_LIN: &[Axis] = &[
    Axis::X, Axis::Y, Axis::Z,
];

pub const AXIS_ROT: &[Axis] = &[
    Axis::RX, Axis::RY, Axis::RZ
];

#[derive(Clone, PartialEq, Debug)]
pub struct AxisCollection<T> {
    x: T,
    y: T,
    z: T,
    rx: T,
    ry: T,
    rz: T,
}

impl <T>AxisCollection<T> {
    pub fn with_axis(f: impl Fn(Axis)->T) -> Self {
        Self{
            x: f(Axis::X),
            y: f(Axis::Y),
            z: f(Axis::Z),
            rx: f(Axis::RX),
            ry: f(Axis::RY),
            rz: f(Axis::RZ),
        }
    }
}

impl <T: Default> Default for AxisCollection<T> {
    fn default() -> Self {
        Self { x: Default::default(), y: Default::default(), z: Default::default(), rx: Default::default(), ry: Default::default(), rz: Default::default() }
    }
}

impl <T> Index<Axis> for AxisCollection<T> {
    type Output = T;

    fn index(&self, index: Axis) -> &Self::Output {
        match index {
            Axis::X =>  &self.x,
            Axis::Y =>  &self.y,
            Axis::Z =>  &self.z,
            Axis::RX => &self.rx, 
            Axis::RY => &self.ry, 
            Axis::RZ => &self.rz,
        }
    }
}

impl <T> IndexMut<Axis> for AxisCollection<T> {
    fn index_mut(&mut self, index: Axis) -> &mut Self::Output {
        match index {
            Axis::X =>  &mut self.x,
            Axis::Y =>  &mut self.y,
            Axis::Z =>  &mut self.z,
            Axis::RX => &mut self.rx, 
            Axis::RY => &mut self.ry, 
            Axis::RZ => &mut self.rz,
        }
    }   
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        // TODO: commands to run setup go here
        (Self{
            values: Default::default(),
            scales: AxisCollection::with_axis(|_| 0.5),
            scale_slider: Default::default(),
            value_slider: Default::default(),
            cgs: AxisCollection::with_axis(|a| CurveGraph{ a, s: 0.5, x: 0.0 }),

            pick_axis: Default::default(),
            axis: Axis::X,
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
            Message::ScaleChanged(a, v) => {
                self.scales[a] = v;
                self.cgs[a].s = v;
            },
            Message::ValueChanged(a, v) => {
                self.values[a] = v;
                self.cgs[a].x = v;
            },
            Message::SelectAxis(a) => {
                self.axis = a;
            },
            _ => (),
        }

        // TODO: Command::perform to call futures

        Command::none()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        iced::Subscription::from_recipe(Idk{})
    }

    fn view(&mut self) -> iced::Element<'_, Self::Message> {

        let mut column_lin = Column::new().padding(20);
        for a in AXIS_LIN {
            let g = Canvas::new(self.cgs[*a].clone())
                .width(Length::Units(200))
                .height(Length::Units(200));

            let row = Row::new().padding(10)
                .push(g);

            column_lin = column_lin.push(Container::new(row).padding(10));
        }

        let axis = self.axis;

        let column_ctrl = Column::new().padding(10)
        .push(ProgressBar::new(0.0..=1.0, self.scales[self.axis]))
        .push(
            Slider::new(
                &mut self.scale_slider[axis],
                0.0..=1.0,
                self.scales[axis],
                move |x| Message::ScaleChanged(axis, x),
            )
            .step(0.01),
        )
        .push(ProgressBar::new(-1.0..=1.0, self.values[axis]))
        .push(
            Slider::new(
                &mut self.value_slider[axis],
                -1.0..=1.0,
                self.values[self.axis],
                move |x| Message::ValueChanged(axis, x),
            )
            .step(0.01),
        )
        .push(PickList::new(
            &mut self.pick_axis,
            AXIS,
            Some(self.axis),
            Message::SelectAxis,
        ));
    
        Row::new().padding(10)
            .push(column_lin)
            .push(column_ctrl)
            .into()
    }
}

struct Idk {

}

impl <H, I> Recipe<H, I>  for Idk 
where
    H: std::hash::Hasher,
{
    type Output = Message;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        "whatever".hash(state)
    }

    fn stream(
        self: Box<Self>,
        input: BoxStream<I>,
    ) -> BoxStream<Self::Output> {
        Box::pin(async_std::stream::interval(Duration::from_millis(100)).map(|_| Message::Tick))
    }
}

#[derive(Clone)]
pub struct CurveGraph {
    pub a: Axis,
    pub s: f32,
    pub x: f32,
}

const N: isize = 100;

impl canvas::Program<Message> for CurveGraph {
    fn draw(&self, bounds: iced::Rectangle, _cursor: canvas::Cursor) -> Vec<canvas::Geometry> {
        let mut f = Frame::new(bounds.size());
        let center = f.center();
        let b = bounds.size();

        let bx = bounds.size().width / 2.0 - 10.0;
        let by = bounds.size().height / 2.0 - 10.0;

        // Setup stroke type
        let mut thin_stroke = Stroke {
            width: 2.0,
            color: Color::BLACK,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };

        // Bounding box
        let p = Path::rectangle(Point::new(1.0, 1.0), Size::new(b.width-2.0, b.height-2.0));
        f.stroke(&p, thin_stroke);

        // Title
        let t = Text{
            content: self.a.to_string(),
            position: Point::new(10.0, 10.0),
            size: 25.0,
            ..Default::default()
        };

        f.fill_text(t);

        // Axes

        thin_stroke.color = Color::from_rgb8(0xDC, 0xDC, 0xDC);
        let p = Path::line(Point{x: bx, y: 0.0}, Point{x: -bx, y: 0.0});
        f.with_save(|f| {
            f.translate(Vector::new(center.x, center.y));
            f.stroke(&p, thin_stroke);
        });

        let p = Path::line(Point{x: 0.0, y: -by}, Point{x: 0.0, y: by});
        f.with_save(|f| {
            f.translate(Vector::new(center.x, center.y));
            f.stroke(&p, thin_stroke);
        });

        thin_stroke.color = Color::BLACK;

        let p = Path::new(|b| {
            let mut last = Point{ x: -bx, y: -by };

            for i in -N..N+1 {
                let x = i as f32 / N as f32;
                let y = self.s * x.powi(3) + (1.0 - self.s) * x;

                let p = Point{ x: x * bx, y: y * -by };

                b.quadratic_curve_to(last, p);


                //println!("x: {:?} y: {:?}", x, y);
                //println!("prev: {:?} next: {:?}", last, next);

                last = p;
            }
        });

        f.with_save(|f| {
            f.translate(Vector::new(center.x, center.y));
            f.stroke(&p, thin_stroke);
        });



        // Center marker
        let y = self.s * self.x.powi(3) + (1.0 - self.s) * self.x;
        let p = Point{ x: self.x * bx, y: y * -by};
        let circle = Path::circle(p, 5.0);

        f.with_save(|f| {
            f.translate(Vector::new(center.x, center.y));
            f.fill(&circle, Color::from_rgb8(0x12, 0x93, 0xD8));
        });


        vec![f.into_geometry()]
    }
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

        let b = self.s.to_bits() ^ self.x.to_bits();

        b.hash(state)
    }
}

impl<'a, M> Into<Element<'a, M>> for CurveGraph{
    fn into(self) -> Element<'a, M> {
        Element::new(self)
    }
}
