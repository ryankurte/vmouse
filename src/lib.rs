use std::collections::HashMap;
use std::str::FromStr;


use serde::{Deserialize, Serialize};
use evdev_rs::enums::{BusType, EventCode, EV_REL};
use evdev_rs::{DeviceWrapper, InputEvent, UInputDevice, UninitDevice};
use log::{debug, trace};


mod command;
pub use command::*;
mod axis;
pub use axis::*;
mod client;
pub use client::*;
mod map;
pub use map::*;
mod config;
pub use config::*;

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
            default: Default::default(),
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
