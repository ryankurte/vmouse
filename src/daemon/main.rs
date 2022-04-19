use std::collections::HashMap;

use std::fs::File;

use std::io::{ErrorKind};
use std::time::Duration;

use async_std::task::JoinHandle;
use evdev_rs::{Device, DeviceWrapper, InputEvent, ReadFlag};
use futures::{stream::StreamExt as _, FutureExt};

use async_std::channel::Sender;
use async_std::os::unix::net::{UnixListener, UnixStream};
use async_std::{io::ReadExt, io::WriteExt};

use structopt::StructOpt;

use log::{debug, warn, error, info, trace, LevelFilter};
use simplelog::{Config as LogConfig, SimpleLogger};

use vmouse::{AxisCollection, AxisValue, Command, Config, UsbDevice};

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {
    /// Socket for daemon connections
    #[structopt(long, default_value = "/var/run/vmouse.sock")]
    pub socket: String,

    /// Configuration file
    #[structopt(long, default_value = "/etc/vmouse/vmouse.toml")]
    pub config: String,

    /// Log verbosity
    #[structopt(long, default_value = "debug")]
    pub log_level: LevelFilter,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    info!("Starting vmousectl");

    let config = Config::default();

    // Attempt to load configuration
    match std::fs::read_to_string(&opts.config) {
        Ok(v) => {

        },
        Err(e) => {
            warn!("Failed to read config '{}': {:?}", opts.config, e);
        },
    }

    debug!("Config: {:?}", config);

    let mut exit = async_ctrlc::CtrlC::new()?.fuse();
    let (ctl_tx, mut ctl_rx) = async_std::channel::unbounded();
    let (evt_tx, mut evt_rx) = async_std::channel::unbounded();
    let (tick_tx, mut tick_rx) = async_std::channel::unbounded::<()>();

    debug!("Connecting to socket: {}", opts.socket);

    // Setup unix listener socket
    let listener = UnixListener::bind(&opts.socket).await?;
    let mut incoming = listener.incoming().fuse();

    let mut d = Daemon::new(config, evt_tx, tick_tx);

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
                    d.attach_client(s, ctl_tx.clone()).await?;
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
                    if let Some((map, val)) = d.config.map(&evt.0, &evt.1) {

                        // If output is enabled, write to virtual device
                        if d.enabled {
                            map.event(&v, evt.1.time, val)?;
                        }
                    }

                    // Update internal state
                    // Convert input event to axis value
                    if let Ok(v) = AxisValue::try_from(evt.1) {
                        d.state[v.a] = v.v;
                    };
                    d.changed = true;

                    // TODO: handle button events
                }
            },
            // Handle tick events
            _t = tick_rx.next() => {
                // Only send on changes
                if !d.changed {
                    continue
                }

                // Periodically push state to connected listeners
                for (_id, c) in d.clients.iter().filter(|(_id, c)| c.listen ) {
                    let tx = c.tx.clone();

                    let _ = async_std::task::spawn(async move {
                        tx.send(Command::State(d.state)).await
                    });
                }

            }
            // Handle exit event
            _e = exit => {
                debug!("Exiting daemon");
                break;
            },
        )
    }

    // Close listener socket
    drop(incoming);
    drop(listener);
    let _ = std::fs::remove_file(&opts.socket);

    Ok(())
}

pub struct Daemon {
    id: u32,
    config: Config,
    state: AxisCollection<f32>,
    evt_tx: Sender<(UsbDevice, InputEvent)>,
    enabled: bool,

    clients: HashMap<u32, ClientHandle>,

    tick_tx: Sender<()>,
    update_task: Option<JoinHandle<()>>,
    changed: bool,
}

impl Daemon {
    fn new(config: Config, evt_tx: Sender<(UsbDevice, InputEvent)>, tick_tx: Sender<()>) -> Self {
        Self {
            id: 0,
            config,
            enabled: true,
            evt_tx,
            tick_tx,
            state: AxisCollection::with_axis(|_| Default::default()),
            clients: Default::default(),
            changed: false,
            update_task: None,
        }
    }

    // Create and attach a new client
    async fn attach_client(
        &mut self,
        mut stream: UnixStream,
        ctl_tx: Sender<CommandHandle>,
    ) -> anyhow::Result<()> {
        let id = self.id;
        self.id = self.id.wrapping_add(1);

        let (resp_tx, mut resp_rx) = async_std::channel::unbounded();
        let tx = resp_tx.clone();

        let mut buff = [0u8; 1024];

        debug!("Spawning task for client: {}", id);

        // Spawn a task for each UnixStream
        let h: JoinHandle<Result<(), anyhow::Error>> = async_std::task::spawn(async move {
            let res = loop {
                futures::select!(
                    // Handle input commands
                    r = stream.read(&mut buff).fuse() => {
                        let r = match r {
                            Ok(n) if n != 0 => &buff[..n],
                            Ok(_n) => break Ok(()),
                            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
                            Err(e) => break Err(e.into()),
                        };

                        debug!("Received: {:02x?}", r);

                        let c: Command = bincode::deserialize(r)?;

                        ctl_tx.send(CommandHandle{id, c, tx: resp_tx.clone()}).await?;
                    },
                    // Forward responses
                    c = resp_rx.next() => {
                        if let Some(c) = c {
                            let enc: Vec<u8> = bincode::serialize(&c)?;

                            trace!("Sending: {:02x?}", enc);

                            stream.write(&enc).await?;
                        } else {
                            break Ok(());
                        }
                    },
                    // TODO: handle stream close
                )
            };

            debug!("Disconnecting from client: {}", id);

            ctl_tx
                .send(CommandHandle {
                    id,
                    c: Command::Disconnect,
                    tx: resp_tx.clone(),
                })
                .await?;

            res
        });

        // Create client handle
        let client = ClientHandle {
            id,
            _h: h,
            tx,
            listen: false,
        };

        // Add client to tracking
        self.clients.insert(client.id, client);

        Ok(())
    }

    async fn enable_update_task(&mut self) {
        if self.update_task.is_none() {
            let tick_tx = self.tick_tx.clone();

            let h = async_std::task::spawn(async move {
                let mut t = async_std::stream::interval(Duration::from_millis(100));

                loop {
                    let _ = t.next().await;
                    let _ = tick_tx.send(()).await;
                }
            });

            self.update_task = Some(h);
        }
    }

    async fn attach_device(&mut self, device: String) -> anyhow::Result<()> {
        // Connect to device
        let f = File::open(&device)?;
        let d = Device::new_from_file(f)?;

        let evt_tx = self.evt_tx.clone();

        // Log device info
        if let Some(n) = d.name() {
            info!(
                "Connected to device: '{}' ({:04x}:{:04x})",
                n,
                d.vendor_id(),
                d.product_id()
            );
        }

        let h = UsbDevice{
            vid: d.vendor_id(),
            pid: d.product_id(),
        };

        // Wrap device in async adapter
        let a = smol::Async::new(d)?;

        // Setup event listening task
        let _h: JoinHandle<Result<(), anyhow::Error>> = async_std::task::spawn(async move {
            let r = loop {
                futures::select!(
                    // Read on incoming events
                    r = a.read_with(|d| d.next_event(ReadFlag::NORMAL)).fuse() => {
                        match r {
                            Ok((_status, evt)) => evt_tx.send((h, evt)).await?,
                            Err(e) => break Err(e.into()),
                        }
                    },
                    // TODO: exit handler
                )
            };

            debug!("Disconnecting from device: {}", device);

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
                    }
                    Err(e) => {
                        error!("Device {} attach failed: {:?}", event, e);
                        Some(Command::Failed)
                    }
                }
            }
            Command::Enable { enabled } => {
                self.enabled = *enabled;
                Some(Command::Ok)
            }
            Command::GetState => Some(Command::State(self.state)),
            Command::GetConfig => Some(Command::Config(self.config.clone())),
            Command::Config(c) => {
                debug!("Updating config: {:?}", c);

                self.config = c.clone();

                Some(Command::Ok)
            }
            Command::Listen => {
                // Set client listen flag
                if let Some(c) = self.clients.get_mut(&h.id) {
                    c.listen = true;
                }

                // Enable update task if required
                self.enable_update_task().await;

                // Signal listen success
                Some(Command::Ok)
            }
            Command::Disconnect => {
                debug!("Removing client: {}", h.id);

                // Remove client from listing
                let _ = self.clients.remove(&h.id);

                // Disable timer task if no longer required
                let listening = self.clients.iter().filter(|(_id, c)| c.listen).count() > 0;
                match (listening, self.update_task.take()) {
                    (false, Some(t)) => {
                        let _ = t.cancel().await;
                    }
                    (_, Some(t)) => self.update_task = Some(t),
                    _ => (),
                }

                None
            }
            _ => None,
        };

        Ok(resp)
    }
}

struct ClientHandle {
    id: u32,
    tx: Sender<Command>,
    listen: bool,
    _h: JoinHandle<Result<(), anyhow::Error>>,
}

struct CommandHandle {
    /// Client ID
    pub id: u32,
    /// Command
    pub c: Command,
    /// Response channel
    pub tx: Sender<Command>,
}
