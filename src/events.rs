use evdev_rs::enums::{EventCode, EventType, EV_KEY, EV_REL, EV_SYN};


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
