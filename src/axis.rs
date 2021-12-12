
use std::ops::{Index, IndexMut};

use evdev_rs::{InputEvent, enums::{EV_REL, EventCode}};
use serde::{Serialize, Deserialize};
use structopt::StructOpt;
use strum_macros::{Display, EnumString, EnumVariantNames};

use crate::AXIS_MAX;

/// Axis kind enumeration
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, EnumString, Display, EnumVariantNames)]
#[derive(Serialize, Deserialize)]
pub enum Axis {
    X, Y, Z, RX, RY, RZ
}

/// List of axes (useful for iteration)
pub const AXIS: &[Axis] = &[
    Axis::X, Axis::Y, Axis::Z, Axis::RX, Axis::RY, Axis::RZ
];

/// List of linear axes (useful for iteration)
pub const AXIS_LIN: &[Axis] = &[
    Axis::X, Axis::Y, Axis::Z,
];

/// List of rotational axes (useful for iteration)
pub const AXIS_ROT: &[Axis] = &[
    Axis::RX, Axis::RY, Axis::RZ
];

/// Helper to convert an EventCode into an [`Axis`] enumeration
impl TryFrom<EventCode> for Axis {
    type Error = ();

    /// Convert an event code into an axis
    fn try_from(value: EventCode) -> Result<Self, Self::Error> {
        let a = match value {
            EventCode::EV_REL(EV_REL::REL_X) =>  Axis::X,
            EventCode::EV_REL(EV_REL::REL_Y) =>  Axis::Y,
            EventCode::EV_REL(EV_REL::REL_Z) =>  Axis::Z,
            EventCode::EV_REL(EV_REL::REL_RX) => Axis::RX,
            EventCode::EV_REL(EV_REL::REL_RY) => Axis::RY,
            EventCode::EV_REL(EV_REL::REL_RZ) => Axis::RZ,
            _ => return Err(())
        };

        Ok(a)
    }
}

/// Axis value type, contains [`Axis`] kind and normalised (-1.0 to 1.0) value
#[derive(Copy, Clone, PartialEq, Debug, StructOpt, Serialize, Deserialize)]
pub struct AxisValue {
    /// Axis associated with value
    pub a: Axis,
    /// Normalised (-1.0 -> 1.0) axis value
    pub v: f32,
}

/// Helper to create an [`AxisValue`] from an evdev [`InputEvent`]
impl TryFrom<InputEvent> for AxisValue {
    type Error = ();

    /// Convert an input event into an AxisValue object
    fn try_from(evt: InputEvent) -> Result<Self, Self::Error> {
        let a = Axis::try_from(evt.event_code)?;
        let v  = evt.value as f32 / AXIS_MAX as f32;
        Ok(Self{ a, v })
    }
}

/// Generic collection of axes with associated values of type T
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AxisCollection<T> {
    pub x: T,
    pub y: T,
    pub z: T,
    pub rx: T,
    pub ry: T,
    pub rz: T,
}

impl <T>AxisCollection<T> {
    /// Constructor to build fields for each axis
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
        Self {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            rx: Default::default(),
            ry: Default::default(),
            rz: Default::default(),
        }
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