use std::collections::HashMap;
use std::os::unix::prelude::AsRawFd;
use std::pin::Pin;
use std::str::FromStr;
use std::task::Poll;



use async_std::os::unix::net::UnixStream;

use futures::{AsyncRead, AsyncWriteExt, Stream};

use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use strum_macros::{Display, EnumVariantNames, EnumString};

use evdev_rs::enums::{BusType, EventCode, EV_REL, EV_SYN};
use evdev_rs::{DeviceWrapper, InputEvent, TimeVal, UInputDevice, UninitDevice};

use log::{debug, trace};

pub mod events;
pub use events::*;
pub mod axis;
pub use axis::*;

#[derive(Clone, PartialEq, Debug, StructOpt, serde::Serialize, serde::Deserialize)]

pub enum Command {
    /// Ping the vmouse daemon (vmoused)
    Ping,
    /// Bind an event input to vmoused
    Bind {
        /// Device event name
        event: String,
    },
    /// Subscribe to events from vmoused
    Listen,

    /// Fetch current state from vmoused
    GetState,

    /// Fetch current config from vmoused
    GetConfig,

    /// Enable or disable vmoused output (useful when changing configuration)
    Enable {
        #[structopt(long)]
        enabled: bool,
    },

    #[structopt(skip)]
    Ok,

    #[structopt(skip)]
    Failed,

    /// Raw value update message
    #[structopt(skip)]
    RawValue(AxisValue),

    /// State update message
    #[structopt(skip)]
    State(AxisCollection<f32>),

    /// Send updated config to vmoused
    #[structopt(skip)]
    SetConfig(Config),

    /// Write updated config to configured file
    WriteConfig,

    /// Signal disconnect for a client
    #[structopt(skip)]
    Disconnect,
}

/// Mouse re-mapping configuration
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub devices: HashMap<UsbDevice, axis::AxisCollection<AxisConfig>>,

    #[serde(default)]
    pub default: axis::AxisCollection<AxisConfig>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    pub devices: Vec<DeviceConfig>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub vid: u16,
    pub pid: u16,

    #[serde(flatten)]
    pub axes: axis::AxisCollection<AxisConfig>,
}

/// Device descriptor object
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct UsbDevice {
    pub vid: u16,
    pub pid: u16,
    pub name: Option<String>,
}

impl ToString for UsbDevice {
    fn to_string(&self) -> String {
        format!("{:04x}:{:04x}", self.vid, self.pid)
    }
}

impl FromStr for UsbDevice {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.split(':');
        
        let (vid, pid) = match (s.next(), s.next()) {
            (Some(vid), Some(pid)) => (vid, pid),
            _ => return Err(()),
        };

        let vid = match u16::from_str_radix(vid, 16) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };

        let pid = match u16::from_str_radix(pid, 16) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };

        Ok(Self{vid, pid, name: None})
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            devices: HashMap::new(),
            default: axis::AxisCollection{
                x: AxisConfig {
                    map: Map::H,
                    scale: 0.005,
                    curve: 0.5,
                    deadzone: 0.0,
                },
                y: AxisConfig {
                    map: Map::V,
                    scale: 0.005,
                    curve: 0.5,
                    deadzone: 0.0,
                },
                z: Default::default(),
                rx: AxisConfig {
                    map: Map::Y,
                    scale: 0.2,
                    curve: 1.0,
                    deadzone: 0.0,
                },
                ry: AxisConfig {
                    map: Map::X,
                    scale: -0.2,
                    curve: 1.0,
                    deadzone: 0.0,
                },
                rz: Default::default(),
            }
        }
    }
}

impl Config {
    pub fn map(&self, d: &UsbDevice, e: &InputEvent) -> Option<(Map, f32)> {
        let axes = match self.devices.get(d) {
            Some(d) => d,
            None => &self.default,
        };

        // Match event codes to configuration
        let m = match e.event_code {
            EventCode::EV_REL(EV_REL::REL_X) => axes.x,
            EventCode::EV_REL(EV_REL::REL_Y) => axes.y,
            EventCode::EV_REL(EV_REL::REL_Z) => axes.z,
            EventCode::EV_REL(EV_REL::REL_RX) => axes.rx,
            EventCode::EV_REL(EV_REL::REL_RY) => axes.ry,
            EventCode::EV_REL(EV_REL::REL_RZ) => axes.rz,
            _ => return None,
        };

        // Normalise input value (AXIS_MIN -> AXIS_MAX to -1.0 -> 1.0)
        let r = e.value as f32 / AXIS_MAX as f32;

        // Apply axis value transformation
        let v = m.transform(r);

        trace!("Map event axis: {} val: {:04} (raw: {:04}", m.map, v, r);

        // Return map and new value
        Some((m.map, v))
    }
}

/// Axis configuration
#[derive(Copy, Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]

pub struct AxisConfig {
    /// Output axis mapping
    pub map: Map,

    /// Output axis sensitivity curve (0.0=x 1.0=x^3)
    pub curve: f32,

    /// Output axis scaling factor
    pub scale: f32,

    /// Output axis deadzone
    pub deadzone: f32,
}

impl Default for AxisConfig {
    fn default() -> Self {
        Self {
            scale: 0.5,
            map: Map::None,
            curve: 0.0,
            deadzone: 0.0,
        }
    }
}

impl AxisConfig {
    /// Apply transformation to raw (-1.0 to 1.0) axis value
    pub fn transform(&self, mut r: f32) -> f32 {
        // Apply deadzones if available
        if r > 0.0 {
            if r < self.deadzone {
                r = 0.0;
            } else {
                r = (r - self.deadzone) / (1.0 - self.deadzone);
            }
        } else if r < 0.0 {
            if r > -self.deadzone {
                r = 0.0;
            } else {
                r = (r + self.deadzone) / (1.0 - self.deadzone);
            }
        }

        // Apply curve / scalar equation if available
        // https://www.chiefdelphi.com/t/paper-joystick-sensitivity-gain-adjustment/107280
        r = self.curve * r.powi(3) + (1.0 - self.curve) * r;

        // Apply scaling if available
        r *= self.scale;

        r
    }
}

/// Output axis function
#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Display,
    EnumString,
    EnumVariantNames,
    serde::Serialize,
    serde::Deserialize,
)]
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

pub const MAPPINGS: &[Map] = &[Map::None, Map::X, Map::Y, Map::H, Map::V];

impl Map {
    pub fn event(&self, v: &UInputDevice, ts: TimeVal, val: f32) -> anyhow::Result<()> {
        // De-normalise value
        let val_i32 = (val * AXIS_MAX as f32) as i32;

        // Write events based on map type
        match self {
            Map::None => return Ok(()),
            Map::X => {
                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_X),
                    value: val_i32,
                })?;
            }
            Map::Y => {
                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_Y),
                    value: val_i32,
                })?;
            }
            Map::H => {
                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL),
                    value: val_i32,
                })?;

                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL_HI_RES),
                    value: (val * AXIS_MAX as f32 * 120.00) as i32,
                })?;
            }
            Map::V => {
                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL),
                    value: -val_i32,
                })?;

                v.write_event(&InputEvent {
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL_HI_RES),
                    value: -(val * AXIS_MAX as f32 * 120.0) as i32,
                })?;
            }
        }

        // Write sync event to commit
        v.write_event(&InputEvent {
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
        u.enable_event_type(t)?;
    }

    for c in EVENT_CODES {
        u.enable_event_code(c, None)?;
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

        Ok(Self { path, stream })
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

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
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
