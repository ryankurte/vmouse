//! Configuration objects and helpers

use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::{UsbDevice, AxisCollection, Map};

/// Mouse re-mapping configuration
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub devices: HashMap<UsbDevice, AxisCollection<AxisConfig>>,

    pub default: AxisCollection<AxisConfig>,
}

impl Config {
    /// Fetch device config by name (`default` or `pid:vid`)
    pub fn get(&self, name: &str) -> Option<&AxisCollection<AxisConfig>> {
        match name {
            "default" => Some(&self.default),
            _ => self.devices.iter().find(|(n, _v)| n.to_string() == name ).map(|(_n, v)| v ),
        }
    }

    /// Fetch device config by name (`default` or `pid:vid`)
    pub fn get_mut(&mut self, name: &str) -> Option<&mut AxisCollection<AxisConfig>> {
        match name {
            "default" => Some(&mut self.default),
            _ => self.devices.iter_mut().find(|(n, _v)| n.to_string() == name ).map(|(_n, v)| v ),
        }
    }

    /// Iterate through configurations
    pub fn iter<'a>(&'a self) -> ConfigIter<'a> {
        ConfigIter {
            config: self,
            index: 0,
        }
    }
}

pub struct ConfigIter<'a> {
    config: &'a Config,
    index: usize,
}

impl <'a> Iterator for ConfigIter<'a> {
    type Item = (String, &'a AxisCollection<AxisConfig>);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        self.index += 1;

        // First return default config
        if index == 0 {
            return Some(("default".to_string(), &self.config.default));
        }

        // Then any specific devices
        self.config.devices.iter().nth(index - 1)
            .map(|(k, v)| (k.to_string(), v))
    }
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
    pub axes: AxisCollection<AxisConfig>,
}


impl Default for AxisCollection<AxisConfig> {
    fn default() -> Self {
        Self {
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

impl Default for AxisCollection<f32> {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
        }
    }
}


/// Axis configuration
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]

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

