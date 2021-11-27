use std::sync::{Arc, Mutex};

use vmouse::{Axis, Client, Command};



#[derive(Clone, Debug)]
pub enum Message {
    ScaleChanged(Axis, f32),
    ValueChanged(Axis, f32),
    SelectAxis(Axis),
    Tick,
    SocketChanged(String),
    Connect,
    Connected(Arc<Mutex<Option<Client>>>),
    Command(Command),
}
