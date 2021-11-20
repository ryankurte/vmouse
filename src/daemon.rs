
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::fs::File;

use std::io::{Read, ErrorKind};
use std::time::Duration;

use async_std::task::JoinHandle;
use evdev_rs::{Device, DeviceWrapper};
use futures::{FutureExt, stream::StreamExt as _};
use async_std::channel::Sender;
use async_std::{io::ReadExt, io::WriteExt};
use async_std::os::unix::net::{UnixListener, UnixStream};

use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};
use vmouse::Command;


#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {

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

    let mut exit = async_ctrlc::CtrlC::new()?.fuse();
    let (ctl_tx, mut ctl_rx) = async_std::channel::unbounded();

    debug!("Connecting to socket: {}", opts.socket);

    // Setup unix listener socket
    let listener = UnixListener::bind(opts.socket).await?;
    let mut incoming = listener.incoming().fuse();

    let mut d = Daemon::new();

    // TODO: setup virtual device

    // TODO: scan for existing devices?

    // Run listen loop
    loop {
        futures::select!(
            // Listen for new unix socket connections
            s = incoming.next() => {
                if let Some(Ok(s)) = s {
                    debug!("New stream!");
                    d.handle_stream(s, ctl_tx.clone()).await?;
                } else {
                    break;
                }
            },
            // Handle control requests
            ctl = ctl_rx.next() => {
                if let Some(CommandHandle{c, tx}) = ctl {
                    debug!("Received command: {:?}", c);
                    if let Some(r) = d.handle_cmd(c)? {
                        tx.send(r).await?;
                    }
                }
            }
            // Handle exit message
            _e = exit => {
                debug!("Exiting daemon");
                break;
            },
        )
    }

    Ok(())
}

pub struct Daemon {
    tasks: Vec<TaskHandle>,
}

impl Daemon {
    fn new() -> Self {
        Self{
            tasks: vec![],
        }
    }

    // Setup a task to handle inputs and forward outputs
    async fn handle_stream(&mut self, mut stream: UnixStream, ctl_tx: Sender<CommandHandle>) -> anyhow::Result<()> {
        let (resp_tx, mut resp_rx) = async_std::channel::unbounded();
        let tx = resp_tx.clone();
        let mut buff = [0u8; 1024];

        let h = async_std::task::spawn(async move {
            loop {
                futures::select!(
                    r = stream.read(&mut buff).fuse() => {
                        let r = match r {
                            Ok(n) => &buff[..n],
                            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
                            Err(e) => return Err(e.into()),
                        };
                
                        debug!("Received: {:02x?}", r);
                
                        let c: Command = bincode::deserialize(r)?;
                
                        ctl_tx.send(CommandHandle{c, tx: resp_tx.clone()}).await?;
                    },
                    c = resp_rx.next() => {
                        if let Some(c) = c {
                            let enc: Vec<u8> = bincode::serialize(&c)?;

                            debug!("Sending: {:02x?}", enc);

                            stream.write(&enc).await?;
                        } else {
                            break;
                        }
                    },
                )
            }

            Ok(())
        });

        self.tasks.push(TaskHandle{h, tx});

        Ok(())
    }

    fn handle_cmd(&mut self, cmd: Command) -> anyhow::Result<Option<Command>> {
        let resp = match cmd {
            Command::Ping => Some(Command::Ok),
            _ => None,
        };

        Ok(resp)
    }
}

fn bind_device(device: &str) -> anyhow::Result<()> {
    // Open device file
    let f = File::open(device)?;
    let d = Device::new_from_file(f)?;

    if let Some(n) = d.name() {
        info!("Connected to device: '{}' ({:04x}:{:04x})", 
            n, d.vendor_id(), d.product_id());
    }



    Ok(())
}

struct TaskHandle {
    h: JoinHandle<Result<(), anyhow::Error>>,
    tx: Sender<Command>,
}

struct CommandHandle {
    pub c: Command,
    pub tx: Sender<Command>,
}

