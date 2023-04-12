
use structopt::StructOpt;
use serde::{Serialize, Deserialize};

use super::{AxisValue, AxisCollection, Config};


#[derive(Clone, PartialEq, Debug, StructOpt, Serialize, Deserialize)]

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
