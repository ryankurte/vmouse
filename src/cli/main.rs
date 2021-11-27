
use std::os::unix::net::UnixStream;
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

use futures::StreamExt;
use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};

use vmouse::{Client, Command};


#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {
    #[structopt(subcommand)]
    pub command: Command,

    /// Socket for daemon connections
    #[structopt(long, default_value="/var/run/vmouse.sock")]
    pub socket: String,

    /// Log verbosity
    #[structopt(long, default_value="debug")]
    pub log_level: LevelFilter,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    info!("Starting vmousectl");

    debug!("Connecting to socket: {}", opts.socket);

    // Connect to daemon socket
    let mut client = Client::connect(opts.socket.clone()).await?;

    debug!("Writing command: {:?}", opts.command);

    // Write command
    client.send(opts.command.clone()).await?;

    // Await response
    let r = client.next().await;

    debug!("Received response: {:?}", r);

    Ok(())
}
