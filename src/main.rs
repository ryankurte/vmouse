
use std::fs::File;
use std::thread;


use structopt::StructOpt;

use log::{LevelFilter, debug, info};
use simplelog::{SimpleLogger, Config as LogConfig};

use evdev_rs::{Device, UninitDevice, DeviceWrapper, ReadFlag, UInputDevice};
use evdev_rs::enums::{EventCode, BusType, EV_REL, EV_KEY, EV_SYN};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use vmouse::{Map, Config};
use vmouse::{EVENT_TYPES, EVENT_CODES, AXIS_MAX, AXIS_MIN};


#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct Options {

    #[structopt(long, default_value="/dev/input/event14")]
    pub device: String,

    #[structopt(long, default_value="info")]
    pub log_level: LevelFilter,
}




fn pb_new(mp: &mut MultiProgress, name: &str) -> ProgressBar {
    let style = ProgressStyle::default_bar().template("[{prefix}] [{bar:40.cyan/blue}] ({msg})").progress_chars("-|-");

    let pb = mp.add(ProgressBar::new((AXIS_MAX - AXIS_MIN + 2) as u64))
        .with_style(style.clone())
        .with_prefix(name.to_string());

    pb.set_position((1 - AXIS_MIN) as u64);
    pb
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let opts = Options::from_args();

    // Setup logging
    let _ = SimpleLogger::init(opts.log_level, LogConfig::default());

    // TODO: list and match on devices

    info!("Connecting to spacemouse device");

    // Connect to device
    let f = File::open(opts.device)?;
    let d = Device::new_from_file(f)?;

    if let Some(n) = d.name() {
        info!("Connected to device: '{}' ({:04x}:{:04x})", 
            n, d.vendor_id(), d.product_id());
    }

    info!("Creating virtual device");

    //v.set_name("Virtual SpaceMouse");
    //v.set_bustype(BusType::BUS_VIRTUAL as u16);

    let u = UninitDevice::new().unwrap();

    u.set_name("Virtual SpaceMouse");
    u.set_bustype(BusType::BUS_USB as u16);
    u.set_vendor_id(0xabcd);
    u.set_product_id(0xefef);

    // https://stackoverflow.com/a/64559658/6074942
    for t in EVENT_TYPES {
        u.enable_event_type(&t)?;
    }

    for c in EVENT_CODES {
        u.enable_event_code(&c, None)?;
    }

    // Attach virtual device to uinput file
    //let v = v.set_file(f)?;

    let v = UInputDevice::create_from_device(&u)?;
    info!("Created virtual device: {}", v.devnode().unwrap());

    let cfg = Config::default();

    let mut mp = MultiProgress::with_draw_target(ProgressDrawTarget::stdout_with_hz(30));

    let pb_x = pb_new(&mut mp, " X");
    let pb_y = pb_new(&mut mp, " Y");
    let pb_z = pb_new(&mut mp, " Z");

    let pb_rx = pb_new(&mut mp, "RX");
    let pb_ry = pb_new(&mut mp, "RY");
    let pb_rz = pb_new(&mut mp, "RZ");

    // Thread to join on progress bars otherwise these do not execute
    thread::spawn(move || mp.join() );

    // Wrap device in async adapter
    let a = smol::Async::new(d)?;

    loop {
        // Await input event

        // Read next input event
        let (_status, event) = a.read_with(|d| {
            d.next_event(ReadFlag::NORMAL)
        }).await?;

        debug!("Event ({}.{:03}) {:?}: {:?}", event.time.tv_sec, event.time.tv_usec, event.event_code, event.value);

        let pb = match event.event_code {
            EventCode::EV_REL(EV_REL::REL_X) =>  Some(&pb_x),
            EventCode::EV_REL(EV_REL::REL_Y) =>  Some(&pb_y),
            EventCode::EV_REL(EV_REL::REL_Z) =>  Some(&pb_z),
            EventCode::EV_REL(EV_REL::REL_RX) => Some(&pb_rx),
            EventCode::EV_REL(EV_REL::REL_RY) => Some(&pb_ry),
            EventCode::EV_REL(EV_REL::REL_RZ) => Some(&pb_rz),
            _ => None,
        };

        if let Some(pb) = pb {
            pb.set_position((event.value - AXIS_MIN + 1) as u64);
            pb.set_message(format!("{:4}", event.value));
        }

        if let Some((map, val)) = cfg.map(&event) {
            map.event(&v, event.time, val)?;
        }
    }

    Ok(())
}
