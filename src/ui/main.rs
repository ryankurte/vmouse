use std::{
    hash::Hash,
    sync::{Arc, Mutex},
};

use futures::{stream::BoxStream, StreamExt};

use iced::{
    alignment::{self, Horizontal, Alignment},
    Application,
    widget::Canvas,
    Length,
    Command,
    Settings, Theme,
};
use iced_native::{
    subscription::Recipe,
    widget::{
        Button, Column, PickList, ProgressBar, Row, Slider, Text, TextInput,
    },
};

use structopt::StructOpt;
use log::{debug, error, info, LevelFilter};
use simplelog::SimpleLogger;

use vmouse::{Axis, AxisCollection, Client, Config, AXIS, AXIS_LIN, AXIS_ROT, MAPPINGS};

mod cg;
use cg::CurveGraph;

mod message;
use message::Message;

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {
    #[structopt(long, default_value = "debug")]
    pub log_level: LevelFilter,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let log_config = simplelog::ConfigBuilder::new()
        .add_filter_ignore_str("wgpu_core")
        .add_filter_ignore_str("iced_wgpu")
        .build();

    let _ = SimpleLogger::init(opts.log_level, log_config);

    App::run(Settings::default())?;

    Ok(())
}

struct App {
    values: AxisCollection<f32>,
    scale_text: String,

    cgs: AxisCollection<Arc<CurveGraph>>,

    config: Config,

    device: String,
    axis: Axis,

    socket: String,

    attached: bool,

    client: Option<Client>,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        let socket = "/var/run/vmouse.sock".to_string();

        let config = Config::default();

        // TODO: commands to run setup go here
        (
            Self {
                values: AxisCollection::with_axis(|_| Default::default()),

                scale_text: Default::default(),

                config: Config::default(),

                cgs: AxisCollection::with_axis(|a| {
                    Arc::new(CurveGraph::new(a, config.default[a], 0.0))
                }),

                device: "default".to_string(),
                axis: Axis::X,

                socket: socket.clone(),

                attached: true,

                client: None,
            },
            iced::Command::batch(vec![Self::connect(socket)]),
        )
    }

    fn title(&self) -> String {
        "VMouse GUI".to_string()
    }

    // Handle events
    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match (message, self.client.clone()) {
            (Message::Connect, None) => {
                return Self::connect(self.socket.clone());
            }
            (Message::Connected(client), _) => {
                debug!("Received client, unpacking");
                let c = client.lock().unwrap().take();
                self.client = c.clone();

                if let Some(c) = c {
                    return Self::command(c, vmouse::Command::GetConfig);
                }
            }
            (Message::Disconnect, Some(_)) => {
                let _ = self.client.take();
            }
            (Message::ApplyConfig, Some(c)) => {
                return Self::command(c, vmouse::Command::SetConfig(self.config.clone()));
            }
            (Message::RevertConfig, Some(c)) => {
                return Self::command(c, vmouse::Command::GetConfig);
            }
            (Message::WriteConfig, Some(c)) => {
                return Self::command(c, vmouse::Command::WriteConfig);
            }
            (Message::Attach, Some(c)) => {
                self.attached = true;
                return Self::command(c, vmouse::Command::Enable { enabled: true });
            }
            (Message::Detach, Some(c)) => {
                self.attached = false;
                return Self::command(c, vmouse::Command::Enable { enabled: false });
            }
            (Message::ScaleChanged(_a, s), _) => {
                // Update scale string
                self.scale_text = s;
            }
            (Message::ApplyScale, _) => {
                // Update scale if value is valid
                let v = match self.scale_text.parse::<f32>() {
                    Ok(v) => v,
                    Err(_e) => {
                        error!("Non-numeric scale value: {}", self.scale_text);
                        return iced::Command::none();
                    }
                };

                if v > -10.0 && v < 10.0 {
                    info!("Applying scale {:0.4} for axis: {}", v, self.axis);

                    if let Some(config) = self.config.get_mut(&self.device) {
                        config[self.axis].scale = v;
                        self.cgs[self.axis].set_config(config[self.axis]);
                    }
                    
                } else {
                    error!("Scale value: {:0.4} exceeds maximum range", v);
                }
            }
            (Message::MappingChanged(m), _) => {
                if let Some(config) = self.config.get_mut(&self.device) {
                    config[self.axis].map = m;
                }
            }
            (Message::CurveChanged(a, c), _) => {
                if let Some(config) = self.config.get_mut(&self.device) {
                    config[a].curve = c;
                    self.cgs[a].set_config(config[a]);
                }
            }
            (Message::DeadzoneChanged(a, d), _) => {
                if let Some(config) = self.config.get_mut(&self.device) {
                    config[a].deadzone = d;
                    self.cgs[a].set_config(config[a]);
                }
            }
            (Message::ValueChanged(a, v), _) => {
                self.values[a] = v;
                self.cgs[a].set_value(v);
            }
            (Message::SelectDevice(d), _) => {
                self.device = d;

                let config = self.config.get(&self.device).unwrap_or(&self.config.default);
                // Update curve graphs
                for a in AXIS {
                    self.cgs[*a].set_config(config[*a]);
                }
            }
            (Message::SelectAxis(a), _) => {
                // Clear previous axis selected state
                self.cgs[self.axis].set_selected(false);

                // Update axis
                self.axis = a;

                // Set new axis selected state
                self.cgs[self.axis].set_selected(true);

                // Update scale text for new axis
                let config = self.config.get(&self.device).unwrap_or(&self.config.default);
                self.scale_text = format!("{:0.4}", config[a].scale);
            }
            (Message::SocketChanged(socket), _) => {
                self.socket = socket;
            }

            (Message::Command(vmouse::Command::SetConfig(c)), _) => {
                debug!("Received config: {:?}", c);

                self.config = c;

                let config = self.config.get(&self.device).unwrap_or(&self.config.default);

                // Update curve graphs
                for a in AXIS {
                    self.cgs[*a].set_config(config[*a]);
                }

                self.cgs[self.axis].set_selected(true);

                self.scale_text = format!("{:0.4}", self.config.default[self.axis].scale);
            }
            (Message::Command(vmouse::Command::State(s)), _) => {
                // Update state map
                self.values = s;

                // Update curve graphs
                for a in AXIS {
                    self.cgs[*a].set_value(s[*a]);
                }
            }
            (Message::Command(cmd), _) => {
                debug!("Received command: {:?}", cmd);
            }
            _ => (),
        }

        Command::none()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        if let Some(c) = self.client.clone() {
            iced::Subscription::from_recipe(Idk { client: c })
        } else {
            iced::Subscription::none()
        }
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        let mut column_lin = Column::new()
            .padding(5)
            .height(Length::Fill)
            .width(Length::FillPortion(2));
        for a in AXIS_LIN {
            let cg = self.cgs[*a].clone();
            let g = Canvas::new(cg)
                .width(Length::FillPortion(2))
                .height(Length::FillPortion(2));

            let row = Row::new()
                .padding(10)
                .height(Length::FillPortion(2))
                .push(g);

            column_lin = column_lin.push(row);
        }

        let mut column_rot = Column::new()
            .padding(5)
            .height(Length::Fill)
            .width(Length::FillPortion(2));
        for a in AXIS_ROT {
            let cg = self.cgs[*a].clone();
            let g = Canvas::new(cg)
                .width(Length::FillPortion(2))
                .height(Length::FillPortion(2));

            let row = Row::new()
                .padding(10)
                .height(Length::FillPortion(2))
                .push(g);

            column_rot = column_rot.push(row);
        }

        let axis = self.axis;

        let mut connect_ctl = Row::new().spacing(10).align_items(Alignment::Center).push(
            TextInput::new(
                "socket",
                &self.socket,
                |v| Message::SocketChanged(v),
            )
            .width(Length::FillPortion(2)),
        );
        if self.client.is_none() {
            connect_ctl = connect_ctl.push(
                Button::new(
                    Text::new("connect").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::Connect)
                .width(Length::FillPortion(1)),
            )
        } else {
            connect_ctl = connect_ctl.push(
                Button::new(
                    Text::new("disconnect").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::Disconnect)
                .width(Length::FillPortion(1)),
            )
        }

        let mut config_ctl = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
            .push(
                Button::new(
                    Text::new("apply").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::ApplyConfig)
                .width(Length::FillPortion(1)),
            )
            .push(
                Button::new(
                    Text::new("revert").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::RevertConfig)
                .width(Length::FillPortion(1)),
            )
            .push(
                Button::new(
                    Text::new("write").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::WriteConfig)
                .width(Length::FillPortion(1)),
            );
        if !self.attached {
            config_ctl = config_ctl.push(
                Button::new(
                    Text::new("attach").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::Attach)
                .width(Length::FillPortion(1)),
            )
        } else {
            config_ctl = config_ctl.push(
                Button::new(
                    Text::new("detach").horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::Detach)
                .width(Length::FillPortion(1)),
            )
        }

        let column_ctrl = Column::new()
            .padding(10)
            .spacing(10)
            .height(Length::Fill)
            .width(Length::FillPortion(2))
            // Device selection
            .push(Text::new("Device:").vertical_alignment(alignment::Vertical::Center))
            .push(
                PickList::new(
                    self.config.iter().map(|(n, _c)| n ).collect::<Vec<_>>(),
                    Some(self.device.clone()),
                    Message::SelectDevice,
                )
                .width(Length::Fill),
            )
            // Axis selection
            .push(Text::new("Axis:").vertical_alignment(alignment::Vertical::Center))
            .push(
                PickList::new(
                    AXIS,
                    Some(self.axis),
                    Message::SelectAxis,
                )
                .width(Length::Fill),
            )
            // Current value display
            .push(Text::new("Value:").vertical_alignment(alignment::Vertical::Center))
            .push(ProgressBar::new(-1.0..=1.0, self.values[axis]))
            .push(Row::new().height(Length::Fixed(10.0)))
            // Mapping configuration
            .push(Text::new("Mapping:").vertical_alignment(alignment::Vertical::Center))
            .push(
                PickList::new(
                    MAPPINGS,
                    self.config.get(&self.device).map(|c| c[self.axis].map ),
                    Message::MappingChanged,
                )
                .width(Length::Fill),
            )
            // Scale configuration
            .push(Text::new("Scale:").vertical_alignment(alignment::Vertical::Center))
            .push(
                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(TextInput::new(
                        "scale",
                        &self.scale_text,
                        move |s| Message::ScaleChanged(axis, s),
                    ))
                    .push(
                        Button::new(Text::new("apply"))
                            .on_press(Message::ApplyScale),
                    ),
            )
            // Curve configuration
            .push(Text::new("Curve:").vertical_alignment(alignment::Vertical::Center))
            .push(
                Slider::new(
                    0.0..=1.0,
                    self.config.get(&self.device).map(|c| c[self.axis].curve ).unwrap_or_default(),
                    move |x| Message::CurveChanged(axis, x),
                )
                .step(0.01),
            )
            // Deadzone configuration
            .push(Text::new("Deadzone:").vertical_alignment(alignment::Vertical::Center))
            .push(
                Slider::new(
                    0.0..=1.0,
                    self.config.get(&self.device).map(|c| c[self.axis].deadzone ).unwrap_or_default(),
                    move |d| Message::DeadzoneChanged(axis, d),
                )
                .step(0.01),
            )
            .push(Row::new().height(Length::Fill))
            .push(Text::new("Control:").vertical_alignment(alignment::Vertical::Center))
            .push(config_ctl)
            // Daemon connection
            .push(Text::new("Socket:").vertical_alignment(alignment::Vertical::Center))
            .push(connect_ctl);
        Row::new()
            .padding(10)
            .push(column_lin)
            .push(column_rot)
            .push(column_ctrl)
            .into()
    }
}

impl App {
    fn connect(socket: String) -> Command<Message> {
        Command::perform(
            async move {
                debug!("Connecting to socket: {}", socket);
                let mut c = Client::connect(socket).await?;

                debug!("Subscribing to socket events");
                c.send(vmouse::Command::Listen).await?;

                debug!("Subscribe ok!");

                Ok(c)
            },
            |r: Result<Client, anyhow::Error>| match r {
                Ok(c) => Message::Connected(Arc::new(Mutex::new(Some(c)))),
                Err(e) => {
                    error!("Connection failed: {:?}", e);
                    Message::Tick
                }
            },
        )
    }

    fn command(mut client: Client, cmd: vmouse::Command) -> Command<Message> {
        Command::perform(
            async move {
                debug!("Issuing config get request");
                client.send(cmd).await?;
                Ok(())
            },
            |r: Result<(), anyhow::Error>| match r {
                Ok(_c) => Message::Tick,
                Err(e) => {
                    error!("Connection failed: {:?}", e);
                    Message::Tick
                }
            },
        )
    }
}

struct Idk {
    client: Client,
}

impl<H, I> Recipe<H, I> for Idk
where
    H: std::hash::Hasher,
{
    type Output = Message;

    fn hash(&self, state: &mut H) {
        self.client.hash(state)
    }

    fn stream(self: Box<Self>, _input: BoxStream<I>) -> BoxStream<Self::Output> {
        Box::pin(self.client.map(|r| match r {
            Ok(v) => Message::Command(v),
            Err(_e) => Message::Tick,
        }))
    }
}
