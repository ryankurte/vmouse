
use std::os::unix::net::UnixStream;
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};

use vmouse::Command;


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


fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    info!("Starting vmousectl");

    debug!("Connecting to socket: {}", opts.socket);

    // Connect to daemon socket
    let mut stream = UnixStream::connect(opts.socket)?;
    stream.set_read_timeout(Some(Duration::from_millis(1000)))?;

    // Encode command
    let encoded: Vec<u8> = bincode::serialize(&opts.command)?;

    debug!("Writing command: {:?} ({:02x?})", opts.command, encoded);

    // Write command
    let _n = stream.write_all(&encoded)?;

    let mut buff = [0u8; 1024];

    // Await response
    let n = stream.read(&mut buff)?;

    // Decode response
    let decoded: Command = bincode::deserialize(&buff[..n])?;

    debug!("Received response: {:?}", decoded);

    Ok(())
}
