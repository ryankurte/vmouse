
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::fs::File;

use std::io::{Read, ErrorKind};
use std::time::Duration;

use async_std::task::JoinHandle;
use evdev_rs::{Device, DeviceWrapper, UInputDevice, InputEvent, ReadFlag};
use futures::{FutureExt, stream::StreamExt as _};


use async_std::channel::Sender;
use async_std::{io::ReadExt, io::WriteExt};
use async_std::os::unix::net::{UnixListener, UnixStream};

use structopt::StructOpt;

use log::{LevelFilter, trace, debug, info, error};
use simplelog::{SimpleLogger, Config as LogConfig};

use vmouse::{Command, Config, AxisValue};


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
    let (evt_tx, mut evt_rx) = async_std::channel::unbounded();

    debug!("Connecting to socket: {}", opts.socket);

    // Setup unix listener socket
    let listener = UnixListener::bind(opts.socket).await?;
    let mut incoming = listener.incoming().fuse();

    let config = Config::default();

    let mut d = Daemon::new(config, evt_tx);

    // Setup virtual device
    let v = vmouse::virtual_device()?;

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
                if let Some(h) = ctl {
                    debug!("Received command: {:?}", h.c);
                    if let Some(r) = d.handle_cmd(&h).await? {
                        h.tx.send(r).await?;
                    }
                }
            },
            // Handle input events
            evt = evt_rx.next() => {
                if let Some(evt) = evt {
                    trace!("Input event: {:?}", evt);

                    // Map input to output event
                    // TODO: multi-device and reconfigurable mappings?
                    if let Some((map, val)) = d.config.map(&evt) {
                        map.event(&v, evt.time, val)?;
                    }

                    // Convert input event to axis value
                    if let Ok(v) = AxisValue::try_from(evt) {
                        

                        // Write to connected listeners
                        for tx in &d.listeners {
                            let tx = tx.clone();
                            let v = v.clone();

                            let _ = async_std::task::spawn(async move {
                                tx.send(Command::RawValue(v)).await
                            });
                        }

                    };

                    // TODO: handle button events
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
    config: Config,
    evt_tx: Sender<InputEvent>,
    listeners: Vec<Sender<Command>>,
}

impl Daemon {
    fn new(config: Config, evt_tx: Sender<InputEvent>) -> Self {
        Self{
            config,
            evt_tx,
            listeners: vec![],
        }
    }

    // Setup a task to handle inputs and forward outputs
    async fn handle_stream(&mut self, mut stream: UnixStream, ctl_tx: Sender<CommandHandle>) -> anyhow::Result<()> {
        let (resp_tx, mut resp_rx) = async_std::channel::unbounded();
        let tx = resp_tx.clone();
        let mut buff = [0u8; 1024];

        // Spawn a task for each UnixStream
        let _h: JoinHandle<Result<(), anyhow::Error>> = async_std::task::spawn(async move {
            loop {
                futures::select!(
                    // Handle input commands
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
                    // Forward responses
                    c = resp_rx.next() => {
                        if let Some(c) = c {
                            let enc: Vec<u8> = bincode::serialize(&c)?;

                            trace!("Sending: {:02x?}", enc);

                            stream.write(&enc).await?;
                        } else {
                            break;
                        }
                    },
                    // TODO: handle exit events
                )
            }

            Ok(())
        });

        Ok(())
    }

    async fn attach_device(&mut self, device: String) -> anyhow::Result<()> {

        // Connect to device
        let f = File::open(&device)?;
        let d = Device::new_from_file(f)?;

        let evt_tx = self.evt_tx.clone();

        // Log device info
        if let Some(n) = d.name() {
            info!("Connected to device: '{}' ({:04x}:{:04x})", 
                n, d.vendor_id(), d.product_id());
        }

        // Wrap device in async adapter
        let a = smol::Async::new(d)?;

        // Setup event listening task
        let h: JoinHandle<Result<(), anyhow::Error>> = async_std::task::spawn(async move {
            let r = loop {
                futures::select!(
                    // Read on incoming events
                    r = a.read_with(|d| d.next_event(ReadFlag::NORMAL)).fuse() => {
                        match r {
                            Ok((_status, evt)) => evt_tx.send(evt).await?,
                            Err(e) => break Err(e.into()),
                        }
                    },
                    // TODO: exit handler
                )
            };

            debug!("Disconnected from device: {}", device);

            r
        });

        Ok(())
    }

    async fn handle_cmd(&mut self, h: &CommandHandle) -> anyhow::Result<Option<Command>> {
        let resp = match &h.c {
            Command::Ping => Some(Command::Ok),
            Command::Bind { event } => {
                info!("Binding device: {}", event);
                match self.attach_device(event.clone()).await {
                    Ok(_) => {
                        info!("Device {} attach OK!", event);
                        Some(Command::Ok)
                    },
                    Err(e) => {
                        error!("Device {} attach failed: {:?}", event, e);
                        Some(Command::Failed)
                    }
                }
            },
            Command::Listen => {
                self.listeners.push(h.tx.clone());
                Some(Command::Ok)
            },
            _ => None,
        };

        Ok(resp)
    }
}

struct DeviceHandle {
    h: JoinHandle<Result<(), anyhow::Error>>,
}

struct TaskHandle {
    h: JoinHandle<Result<(), anyhow::Error>>,
    tx: Sender<Command>,
}

struct CommandHandle {
    pub c: Command,
    pub tx: Sender<Command>,
}

