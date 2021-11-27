use std::{hash::Hash, ops::{Index, IndexMut}, sync::{Arc, Mutex}, time::Duration};

use futures::{StreamExt, stream::BoxStream};
use iced::{Application, Canvas, Column, Command, Container, Element, Length, PickList, Point, ProgressBar, Row, Settings, Size, Slider, Text, TextInput, Vector, pick_list, slider, text_input};
use iced::canvas::{self, Cache, LineCap, Path, Stroke};
use iced_native::{Color, Widget, layout, renderer, subscription::Recipe, widget::{Button, button}};

use structopt::StructOpt;
use strum_macros::Display;

use log::{LevelFilter, debug, info, error};
use simplelog::{SimpleLogger, Config as LogConfig};

use vmouse::{AXIS, AXIS_LIN, AXIS_ROT, Axis, AxisCollection, Client};

mod cg;
use cg::CurveGraph;

mod message;
use message::Message;

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {

    #[structopt(long, default_value="debug")]
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
    cgs: AxisCollection<Arc<CurveGraph>>,

    pick_axis: pick_list::State<Axis>,
    axis: Axis,

    socket_state: text_input::State,
    socket: String,

    connect_state: button::State,
    
    client: Option<Client>,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let socket = "/var/run/vmouse.sock".to_string();
        // TODO: commands to run setup go here
        (Self{
            values: Default::default(),
            scales: AxisCollection::with_axis(|_| 0.5),
            scale_slider: Default::default(),
            value_slider: Default::default(),
            cgs: AxisCollection::with_axis(|a| Arc::new(CurveGraph::new(a, 0.5, 0.0))),

            pick_axis: Default::default(),
            axis: Axis::X,

            socket_state: Default::default(),
            socket: socket.clone(),

            connect_state: Default::default(),
            client: None,
        }, Self::connect(socket.clone()))
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
            Message::ScaleChanged(a, s) => {
                self.scales[a] = s;
                self.cgs[a].set_scale(s);
            },
            Message::ValueChanged(a, v) => {
                self.values[a] = v;
                self.cgs[a].set_value(v);
            },
            Message::SelectAxis(a) => {
                self.axis = a;
            },
            Message::SocketChanged(socket) => {
                self.socket = socket;
            },
            Message::Connect => {
                let socket = self.socket.clone();
                return Self::connect(self.socket.clone());
            },
            Message::Connected(client) => {
                debug!("Received client, unpacking");
                let c = client.lock().unwrap().take();
                self.client = c;
            },
            Message::Command(vmouse::Command::RawValue(v)) => {
                self.values[v.a] = v.v;
                self.cgs[v.a].set_value(v.v);
            },
            Message::Command(cmd) => {
                debug!("Received command: {:?}", cmd);         
            },
            _ => (),
        }

        // TODO: Command::perform to call futures

        Command::none()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        if let Some(c) = self.client.clone() {
            iced::Subscription::from_recipe(Idk{client: c})
        } else {
            iced::Subscription::none()
        }
    }

    fn view(&mut self) -> iced::Element<'_, Self::Message> {

        let mut column_lin = Column::new().padding(5)
            .height(Length::Fill).width(Length::FillPortion(2));
        for a in AXIS_LIN {
            let g = Canvas::new(self.cgs[*a].clone())
                .width(Length::FillPortion(2))
                .height(Length::FillPortion(2));

            let row = Row::new().padding(10)
                .height(Length::FillPortion(2))
                .push(g);

            column_lin = column_lin.push(row);
        }

        let mut column_rot = Column::new().padding(5)
            .height(Length::Fill).width(Length::FillPortion(2));
        for a in AXIS_ROT {
            let g = Canvas::new(self.cgs[*a].clone())
                .width(Length::FillPortion(2))
                .height(Length::FillPortion(2));

            let row = Row::new().padding(10)
                .height(Length::FillPortion(2))
                .push(g);

            column_rot = column_rot.push(row);
        }

        let axis = self.axis;

        let column_ctrl = Column::new().padding(10)
        .height(Length::Fill).width(Length::FillPortion(2))
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
        ))
        .push(TextInput::new(&mut self.socket_state, "socket", &self.socket, Message::SocketChanged))
        .push(Button::new(&mut self.connect_state, Text::new("connect")).on_press(Message::Connect));
    
        Row::new().padding(10)
            .push(column_lin)
            .push(column_rot)
            .push(column_ctrl)
            .into()
    }
}

impl App {
    fn connect(socket: String) -> Command<Message> {
        Command::perform(async move {
            debug!("Connecting to socket: {}", socket);
            let mut c = Client::connect(socket).await?;
            
            debug!("Subscribing to socket events");
            c.send(vmouse::Command::Listen).await?;

            debug!("Subscribe ok!");

            Ok(c)
        }, |r: Result<Client, anyhow::Error>| {
            match r {
                Ok(c) => Message::Connected(Arc::new(Mutex::new(Some(c)))),
                Err(e) => {
                    error!("Connection failed: {:?}", e);
                    Message::Tick
                }
            }
        })
    }

}

struct Idk {
    client: Client,
}

impl <H, I> Recipe<H, I>  for Idk 
where
    H: std::hash::Hasher,
{
    type Output = Message;

    fn hash(&self, state: &mut H) {
        self.client.hash(state)
    }

    fn stream(
        self: Box<Self>,
        _input: BoxStream<I>,
    ) -> BoxStream<Self::Output> {
        Box::pin(self.client.map(|r| {
            match r {
                Ok(v) => Message::Command(v),
                Err(_e) => Message::Tick,
            }
        }))
    }
}
