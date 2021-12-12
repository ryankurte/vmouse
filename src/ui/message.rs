use std::sync::{Arc, Mutex};

use vmouse::{Axis, Client, Command, Map};

#[derive(Clone, Debug)]
pub enum Message {
    None,
    ScaleChanged(Axis, String),
    ApplyScale,
    CurveChanged(Axis, f32),
    DeadzoneChanged(Axis, f32),
    ValueChanged(Axis, f32),
    MappingChanged(Map),
    SelectAxis(Axis),
    Tick,
    SocketChanged(String),
    Connect,
    Disconnect,
    Connected(Arc<Mutex<Option<Client>>>),
    Command(Command),
    ApplyConfig,
    RevertConfig,
    Attach,
    Detach,
}
