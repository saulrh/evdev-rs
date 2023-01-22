use evdev_rs::enums::*;
use evdev_rs::*;
use std::fs::OpenOptions;
use std::io;
use std::io::ErrorKind;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

fn usage() {
    println!("Usage: evtest /path/to/device");
}

fn print_abs_bits(dev: &Device, axis: &EV_ABS) {
    let code = EventCode::EV_ABS(axis.clone());

    if !dev.has(code) {
        return;
    }

    let abs = dev.abs_info(&code).unwrap();

    println!("\tValue\t{}", abs.value);
    println!("\tMin\t{}", abs.minimum);
    println!("\tMax\t{}", abs.maximum);
    if abs.fuzz != 0 {
        println!("\tFuzz\t{}", abs.fuzz);
    }
    if abs.flat != 0 {
        println!("\tFlat\t{}", abs.flat);
    }
    if abs.resolution != 0 {
        println!("\tResolution\t{}", abs.resolution);
    }
}

fn print_code_bits(dev: &Device, ev_type: &EventType) {
    for code in EventCodeIterator::new(ev_type) {
        if !dev.has(code) {
            continue;
        }

        println!("    Event code: {}", code);
        match code {
            EventCode::EV_ABS(k) => print_abs_bits(dev, &k),
            _ => (),
        }
    }
}

fn print_bits(dev: &Device) {
    println!("Supported events:");

    for ev_type in EventTypeIterator::new() {
        if dev.has(ev_type) {
            println!("  Event type: {} ", ev_type);
        }

        match ev_type {
            EventType::EV_KEY
            | EventType::EV_REL
            | EventType::EV_ABS
            | EventType::EV_LED => print_code_bits(dev, &ev_type),
            _ => (),
        }
    }
}

fn print_props(dev: &Device) {
    println!("Properties:");

    for input_prop in InputPropIterator::new() {
        if dev.has_property(&input_prop) {
            println!("  Property type: {}", input_prop);
        }
    }
}

fn print_event(ev: &InputEvent) {
    match ev.event_code {
        EventCode::EV_SYN(_) => println!(
            "Event: time {}.{}, ++++++++++++++++++++ {} +++++++++++++++",
            ev.time.tv_sec,
            ev.time.tv_usec,
            ev.event_type().unwrap()
        ),
        _ => println!(
            "Event: time {}.{}, type {} , code {} , value {}",
            ev.time.tv_sec,
            ev.time.tv_usec,
            ev.event_type()
                .map(|ev_type| format!("{}", ev_type))
                .unwrap_or("None".to_owned()),
            ev.event_code,
            ev.value
        ),
    }
}

fn print_sync_dropped_event(ev: &InputEvent) {
    print!("SYNC DROPPED: ");
    print_event(ev);
}

fn main() {
    let mut args = std::env::args();

    if args.len() != 2 {
        usage();
        std::process::exit(1);
    }

    let path = &args.nth(1).unwrap();
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    let mut buffer = Vec::new();
    let result = file.read_to_end(&mut buffer);
    if result.is_ok() || result.unwrap_err().kind() != ErrorKind::WouldBlock {
        println!("Failed to drain pending events from device file");
    }

    let u_d = UninitDevice::new().unwrap();
    let d = u_d.set_file(file).unwrap();

    println!(
        "Input device ID: bus 0x{:x} vendor 0x{:x} product 0x{:x}",
        d.bustype(),
        d.vendor_id(),
        d.product_id()
    );
    println!("Evdev version: {:x}", d.driver_version());
    println!("Input device name: \"{}\"", d.name().unwrap_or(""));
    println!("Phys location: {}", d.phys().unwrap_or(""));
    println!("Uniq identifier: {}", d.uniq().unwrap_or(""));

    print_bits(&d);
    print_props(&d);

    let mut a: io::Result<(ReadStatus, InputEvent)>;
    loop {
        a = d.next_event(ReadFlag::NORMAL);
        if a.is_ok() {
            let mut result = a.ok().unwrap();
            match result.0 {
                ReadStatus::Sync => {
                    println!("::::::::::::::::::::: dropped ::::::::::::::::::::::");
                    while result.0 == ReadStatus::Sync {
                        print_sync_dropped_event(&result.1);
                        a = d.next_event(ReadFlag::SYNC);
                        if a.is_ok() {
                            result = a.ok().unwrap();
                        } else {
                            break;
                        }
                    }
                    println!("::::::::::::::::::::: re-synced ::::::::::::::::::::");
                }
                ReadStatus::Success => print_event(&result.1),
            }
        } else {
            let err = a.err().unwrap();
            match err.raw_os_error() {
                Some(libc::EAGAIN) => continue,
                _ => {
                    println!("{}", err);
                    break;
                }
            }
        }
    }
}
