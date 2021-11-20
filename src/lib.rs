
use structopt::StructOpt;
use serde::{Serialize, Deserialize};

use evdev_rs::{TimeVal, InputEvent, UInputDevice};
use evdev_rs::enums::{EventCode, EventType, BusType, EV_REL, EV_KEY, EV_SYN};


/// Enabled event types
pub const EVENT_TYPES: &[EventType] = &[
    EventType::EV_KEY,
    EventType::EV_REL,
];

/// Enabled event codes
pub const EVENT_CODES: &[EventCode] = &[
    EventCode::EV_KEY(EV_KEY::BTN_LEFT),
    EventCode::EV_KEY(EV_KEY::BTN_RIGHT),

    EventCode::EV_REL(EV_REL::REL_X),
    EventCode::EV_REL(EV_REL::REL_Y),
    EventCode::EV_REL(EV_REL::REL_WHEEL),
    EventCode::EV_REL(EV_REL::REL_HWHEEL),
    EventCode::EV_REL(EV_REL::REL_WHEEL_HI_RES),
    EventCode::EV_REL(EV_REL::REL_HWHEEL_HI_RES),

    EventCode::EV_SYN(EV_SYN::SYN_REPORT),
];

pub const AXIS_MAX: i32 = 350;
pub const AXIS_MIN: i32 = -350;



#[derive(Clone, PartialEq, Debug, StructOpt)]
#[derive(serde::Serialize, serde::Deserialize)]

pub enum Command {
    Ping,
    Bind{
        event: String,
        vid: String,
        pid: String,
    },
    Ok,
}


/// Mouse re-mapping configuration
#[derive(Copy, Clone, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Input x axis
    pub x: Axis,
    /// Input y axis
    pub y: Axis,
    /// Input z axis
    pub z: Axis,
    /// Input rx axis
    pub rx: Axis,
    /// Input ry axis
    pub ry: Axis,
    /// Input rz axis
    pub rz: Axis,
}

impl Default for Config {
    fn default() -> Self {
        Self { 
            x: Axis{map: Map::H, scale: 0.5, curve: Some(0.5)}, 
            y: Axis{map: Map::V, scale: 0.5, curve: Some(0.5)}, 
            z: Default::default(), 
            rx: Axis{map: Map::Y, scale: 0.2, curve: Some(1.0)},
            ry: Axis{map: Map::X, scale: -0.2, curve: Some(1.0)}, 
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

pub struct Axis {
    /// Output axis mapping
    pub map: Map,

    /// Output axis sensitivity curve (0.0=x 1.0=x^3)
    pub curve: Option<f32>,

    /// Output axis scaling factor
    pub scale: f32,
}

impl Default for Axis {
    fn default() -> Self {
        Axis {
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
        let val = (val * AXIS_MAX as f32) as i32;

        // Write events based on map type
        match self {
            Map::None => {
                return Ok(())
            },
            Map::X => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_X),
                    value: val,
                })?;
            },
            Map::Y => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_Y),
                    value: val,
                })?;
            },
            Map::H => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL),
                    value: val / 120,
                })?;

                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_HWHEEL_HI_RES),
                    value: val,
                })?;
            },
            Map::V => {
                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL),
                    value: -val / 120,
                })?;

                v.write_event(&InputEvent{
                    time: ts,
                    event_code: EventCode::EV_REL(EV_REL::REL_WHEEL_HI_RES),
                    value: -val,
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
