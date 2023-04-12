use evdev_rs::{enums::{EventType, EventCode, EV_KEY, EV_REL, EV_SYN}, UInputDevice, TimeVal, InputEvent};
use strum::{Display, EnumString, EnumVariantNames};
use serde::{Serialize, Deserialize};

/// Maximum axis value
pub const AXIS_MAX: i32 = 350;
/// Minimum axis value
pub const AXIS_MIN: i32 = -350;

/// Enabled event types
pub const EVENT_TYPES: &[EventType] = &[EventType::EV_KEY, EventType::EV_REL];

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
    Serialize,
    Deserialize,
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

