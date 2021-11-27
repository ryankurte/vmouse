

use std::os::unix::prelude::AsRawFd;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use async_std::channel::{Receiver, Sender};
use async_std::os::unix::net::UnixStream;
use futures::stream::BoxStream;
use futures::{AsyncRead, AsyncWriteExt, Sink, Stream};
use structopt::StructOpt;
use strum_macros::Display;
use serde::{Serialize, Deserialize};

use evdev_rs::{TimeVal, InputEvent, UInputDevice, UninitDevice, DeviceWrapper};
use evdev_rs::enums::{EventCode, EventType, BusType, EV_REL, EV_KEY, EV_SYN};


use log::{trace, debug};


pub mod events;
pub use events::*;
pub mod axis;
pub use axis::*;


#[derive(Clone, PartialEq, Debug, StructOpt)]
#[derive(serde::Serialize, serde::Deserialize)]

pub enum Command {
    Ping,
    Bind{
        /// Device event name
        event: String,
    },
    Ok,
    Listen,
    Failed,
    RawValue(AxisValue),
}


/// Mouse re-mapping configuration
pub type Config = axis::AxisCollection<AxisConfig>;

impl Config {
    pub fn standard() -> Self {
        Self { 
            x: AxisConfig{map: Map::H, scale: 0.005, curve: Some(0.5)}, 
            y: AxisConfig{map: Map::V, scale: 0.005, curve: Some(0.5)}, 
            z: Default::default(), 
            rx: AxisConfig{map: Map::Y, scale: 0.2, curve: Some(1.0)},
            ry: AxisConfig{map: Map::X, scale: -0.2, curve: Some(1.0)}, 
            rz: Default::default(),
        }
    }
}

impl Config {
    pub fn map(&self, e: &InputEvent) -> Option<(Map, f32)> {
        // Match event codes to configuration
        let m = match e.event_code {
            EventCode::EV_REL(EV_REL::REL_X) =>  self.x,
            EventCode::EV_REL(EV_REL::REL_Y) =>  self.y,
            EventCode::EV_REL(EV_REL::REL_Z) =>  self.z,
            EventCode::EV_REL(EV_REL::REL_RX) => self.rx,
            EventCode::EV_REL(EV_REL::REL_RY) => self.ry,
            EventCode::EV_REL(EV_REL::REL_RZ) => self.rz,
            _ => return None,
        };

        // Normalise input value (AXIS_MIN -> AXIS_MAX to -1.0 -> 1.0)
        let mut v = e.value as f32 / AXIS_MAX as f32;

        // Apply curve / scalar equation if available
        // https://www.chiefdelphi.com/t/paper-joystick-sensitivity-gain-adjustment/107280
        if let Some(c) = m.curve {
            v = c * v.powi(3) + (1.0 - c) * v;
        }

        // Apply scaling if available
        v *= m.scale;
                
        // Return map and new value
        Some((m.map, v))
    }
}

/// Axis configuration
#[derive(Copy, Clone, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]

pub struct AxisConfig {
    /// Output axis mapping
    pub map: Map,

    /// Output axis sensitivity curve (0.0=x 1.0=x^3)
    pub curve: Option<f32>,

    /// Output axis scaling factor
    pub scale: f32,
}

impl Default for AxisConfig {
    fn default() -> Self {
        Self {
            scale: 0.5,
            map: Map::None,
            curve: None,
        }
    }
}

/// Output axis function
#[derive(Copy, Clone, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Map {
    /// Unmapped
    None,
    /// X axis
    X,
    /// Y axis
    Y,
    /// H axis (horizontal scroll)
    H,
    /// V axis (vertical scroll)
    V,
}


impl Map {
    pub fn event(&self, v: &UInputDevice, ts: TimeVal, val: f32) -> anyhow::Result<()> {

        // De-normalise value
        let val_i32 = (val * AXIS_MAX as f32) as i32;

        // Write events based on map type
        match self {
            Map::None => {
                return Ok(())
            },
            Map::X => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_X),
                    value: val_i32,
                })?;
            },
            Map::Y => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_Y),
                    value: val_i32,
                })?;
            },
            Map::H => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL),
                    value: val_i32,
                })?;

                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL_HI_RES),
                    value: (val * AXIS_MAX as f32 * 120.00) as i32,
                })?;
            },
            Map::V => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL),
                    value: -val_i32,
                })?;

                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL_HI_RES),
                    value: -(val * AXIS_MAX as f32 * 120.0) as i32,
                })?;
            },
        }

        // Write sync event to commit
        v.write_event(&InputEvent{
            time: ts,
            event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
            value: 0,
        })?;

        Ok(())
    }
}

pub fn virtual_device() -> Result<UInputDevice, anyhow::Error> {
    let u = UninitDevice::new().unwrap();

    u.set_name("Virtual SpaceMouse");
    u.set_bustype(BusType::BUS_USB as u16);
    u.set_vendor_id(0xabcd);
    u.set_product_id(0xefef);

    // https://stackoverflow.com/a/64559658/6074942
    for t in EVENT_TYPES {
        u.enable_event_type(&t)?;
    }

    for c in EVENT_CODES {
        u.enable_event_code(&c, None)?;
    }

    // Attach virtual device to uinput file
    //let v = v.set_file(f)?;

    let v = UInputDevice::create_from_device(&u)?;
    debug!("Created virtual device: {}", v.devnode().unwrap());

    Ok(v)
}

#[derive(Clone, Debug)]
pub struct Client {
    path: String,
    stream: UnixStream,
}

impl Client {
    pub async fn connect(path: String) -> Result<Self, std::io::Error> {
        // Connect to daemon socket
        let stream = UnixStream::connect(&path).await?;
        
        Ok(Self{path, stream})
    }

    pub async fn send(&mut self, cmd: Command) -> Result<(), anyhow::Error> {
        let encoded: Vec<u8> = bincode::serialize(&cmd)?;

        debug!("Send: {:?}", cmd);
        
        let _n = self.stream.write_all(&encoded).await?;
        
        Ok(())
    }
}

impl Stream for Client {
    type Item = Result<Command, anyhow::Error>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buff = vec![0u8; 1024];

        let n = match Pin::new(&mut self.stream).poll_read(cx, &mut buff) {
            Poll::Ready(Ok(n)) => n,
            Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e.into()))),
            Poll::Pending => return Poll::Pending,
        };

        let decoded: Command = match bincode::deserialize(&buff[..n]) {
            Ok(d) => d,
            Err(e) => return Poll::Ready(Some(Err(e.into()))),
        };

        trace!("Receive: {:?}", decoded);

        Poll::Ready(Some(Ok(decoded)))
    }
}

impl std::hash::Hash for Client {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.stream.as_raw_fd().hash(state);
    }
}