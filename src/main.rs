mod ble;
mod char_db;
mod cycle_tree;
mod display;
mod fit;
mod inky_phat;
mod peripherals;
mod workout;

use ble::{
    csc_measurement::{checked_rpm_and_new_count, parse_csc_measurement, CscMeasurement},
    cycling_power_measurement::{
        parse_cycling_power_measurement, AccumulatedTorqueSource, CyclingPowerMeasurement,
    },
    heart_rate_measurement::parse_hrm,
};
use btleplug::api::{Central, CentralEvent, Peripheral, UUID};
use btleplug::bluez::manager::Manager;
use peripherals::kickr::Kickr;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{stdout, Write};
use std::mem;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use workout::{run_workout, single_value};

pub fn main() {
    env_logger::init();

    let args: BTreeSet<String> = env::args().collect();
    let use_hr = args.is_empty() || args.contains("--hr");
    let use_cadence = args.is_empty() || args.contains("--cadence");
    let use_power = args.is_empty() || args.contains("--power");
    let is_output_mode = args.is_empty() || args.contains("--output");
    if !use_hr && !use_cadence && !use_power && !is_output_mode {
        panic!("No metrics/mode selected!");
    }

    let db = char_db::open_default().unwrap();

    if is_output_mode {
        // TODO: Should accept a cli flag for output mode vs session mode
        let most_recent_session = db.get_most_recent_session().unwrap().unwrap();
        File::create("workout.fit")
            .unwrap()
            .write_all(&db_session_to_fit(&db, most_recent_session)[..])
            .unwrap();
    } else {
        // We want instant, because we want this to be monotonic. We don't want
        // clock drift/corrections to cause events to be processed out of order.
        let start = Instant::now();

        // Create Our Display
        let display_mutex = Arc::new(Mutex::new(display::Display::new(Instant::now())));

        // This won't fail unless the clock is before epoch, which sounds like a
        // bigger problem
        let session_key = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        println!("Getting Manager...");
        lock_and_show(&display_mutex, &"Getting Started");
        let manager = Manager::new().unwrap();

        let mut adapter = manager.adapters().unwrap().into_iter().next().unwrap();

        adapter = manager.down(&adapter).unwrap();
        adapter = manager.up(&adapter).unwrap();

        let central = adapter.connect().unwrap();
        // There's a bug in 0.4 that does not default the scan to active.
        // Without an active scan the Polar H10 will not give back its name.
        // TODO: remove this line after merge and upgrade.
        central.active(true);

        println!("Starting Scan...");
        lock_and_show(&display_mutex, &"Scanning for Devices");
        central.start_scan().unwrap();

        thread::sleep(Duration::from_secs(5));

        println!("Stopping scan...");
        central.stop_scan().unwrap();
        lock_and_show(&display_mutex, &"Scan Complete! Connecting to Devices.");

        if use_hr {
            // Connect to HRM and print its parsed notifications
            let hrm = central
                .peripherals()
                .into_iter()
                .find(|p| {
                    p.properties()
                        .local_name
                        .iter()
                        .any(|name| name.contains("Polar"))
                })
                .unwrap();
            println!("Found HRM");

            hrm.connect().unwrap();
            println!("Connected to HRM");

            hrm.discover_characteristics().unwrap();
            println!("All characteristics discovered");

            let hr_measurement = hrm
                .characteristics()
                .into_iter()
                .find(|c| c.uuid == UUID::B16(0x2A37))
                .unwrap();

            hrm.subscribe(&hr_measurement).unwrap();
            println!("Subscribed to hr measure");

            let db_hrm = db.clone();
            let display_mutex_hrm = display_mutex.clone();
            hrm.on_notification(Box::new(move |n| {
                let mut display = display_mutex_hrm.lock().unwrap();
                display.update_heart_rate(Some(parse_hrm(&n.value).bpm as u8));
                let elapsed = start.elapsed();
                db_hrm.insert(session_key, elapsed, n).unwrap();
            }));
            lock_and_show(&display_mutex, &"Setup Complete for Heart Rate Monitor");
        }

        if use_power {
            // Connect to Kickr and print its raw notifications
            let kickr = Kickr::new(central.clone()).unwrap();

            let db_kickr = db.clone();
            let display_mutex_kickr = display_mutex.clone();
            let mut last_power_reading = CyclingPowerMeasurement {
                instantaneous_power: 0,
                pedal_power_balance_percent: None,
                accumulated_torque: Some((AccumulatedTorqueSource::Wheel, 0.0)),
                wheel_revolution_data: None,
                crank_revolution_data: None,
            };
            let mut acc_torque = 0.0;
            kickr.on_notification(Box::new(move |n| {
                if n.uuid == UUID::B16(0x2A63) {
                    let mut display = display_mutex_kickr.lock().unwrap();
                    let power_reading = parse_cycling_power_measurement(&n.value);
                    let a = last_power_reading.accumulated_torque.unwrap().1;
                    let b = power_reading.accumulated_torque.unwrap().1;
                    acc_torque = acc_torque + b - a + if a > b { 2048.0 } else { 0.0 };
                    display.update_power(Some(power_reading.instantaneous_power));
                    display.update_external_energy(2.0 * std::f64::consts::PI * acc_torque);
                    last_power_reading = power_reading;
                    let elapsed = start.elapsed();
                    db_kickr.insert(session_key, elapsed, n).unwrap();
                } else {
                    println!("Non-power notification from kickr: {:?}", n);
                }
            }));

            // run our workout
            thread::spawn(move || loop {
                run_workout(Instant::now(), single_value(160), |p| {
                    kickr.set_power(p).unwrap();
                })
            });

            lock_and_show(&display_mutex, &"Setup Complete for Kickr");
        }

        if use_cadence {
            // Connect to Cadence meter and print its raw notifications
            let cadence_measure = central
                .peripherals()
                .into_iter()
                .find(|p| {
                    p.properties()
                        .local_name
                        .iter()
                        .any(|name| name.contains("CADENCE"))
                })
                .unwrap();

            println!("Found CADENCE");

            cadence_measure.connect().unwrap();
            println!("Connected to CADENCE");

            cadence_measure.discover_characteristics().unwrap();
            println!("All characteristics discovered");

            let cadence_measurement = cadence_measure
                .characteristics()
                .into_iter()
                .find(|c| c.uuid == UUID::B16(0x2A5B))
                .unwrap();

            cadence_measure.subscribe(&cadence_measurement).unwrap();
            println!("Subscribed to cadence measure");

            let mut o_last_cadence_measure: Option<CscMeasurement> = None;
            let mut crank_count = 0;
            let db_cadence_measure = db.clone();
            let display_mutex_cadence = display_mutex.clone();
            cadence_measure.on_notification(Box::new(move |n| {
                let elapsed = start.elapsed();
                let csc_measure = parse_csc_measurement(&n.value);
                let last_cadence_measure = mem::replace(&mut o_last_cadence_measure, None);
                if let Some(last_cadence_measure) = last_cadence_measure {
                    let a = last_cadence_measure.crank.unwrap();
                    let b = csc_measure.crank.as_ref().unwrap();
                    if let Some((rpm, new_crank_count)) = checked_rpm_and_new_count(&a, &b) {
                        crank_count = crank_count + new_crank_count;
                        let mut display = display_mutex_cadence.lock().unwrap();
                        display.update_cadence(Some(rpm as u8));
                        display.update_crank_count(crank_count);
                        stdout().flush().unwrap();
                    }
                }
                o_last_cadence_measure = Some(csc_measure);
                db_cadence_measure.insert(session_key, elapsed, n).unwrap();
            }));
            lock_and_show(&display_mutex, &"Setup Complete for Cadence Monitor");
        }

        let central_for_disconnects = central.clone();
        central.on_event(Box::new(move |evt| {
            println!("{:?}", evt);
            match evt {
                CentralEvent::DeviceDisconnected(addr) => {
                    println!("PERIPHERAL DISCONNECTED");
                    let p = central_for_disconnects.peripheral(addr).unwrap();
                    // Kickr is handled on its own
                    if !peripherals::kickr::is_kickr(&p) {
                        thread::sleep(Duration::from_secs(2));
                        p.connect().unwrap();

                        println!("PERIPHERAL RECONNECTED");
                    }
                }
                _ => {}
            }
        }));

        // Update it every second
        let display_mutex_for_render = display_mutex.clone();
        thread::spawn(move || loop {
            let mut display = display_mutex_for_render.lock().unwrap();
            display.render();
        });

        thread::park();
    }
}

fn lock_and_show(display_mutex: &Arc<Mutex<display::Display>>, msg: &str) {
    let mut display = display_mutex.lock().unwrap();
    display.render_msg(msg);
}

fn db_session_to_fit(db: &char_db::CharDb, session_key: u64) -> Vec<u8> {
    let mut last_power: u16 = 0;
    let mut last_csc_measurement: Option<CscMeasurement> = None;
    let mut record: Option<fit::FitRecord> = None;
    let mut records = Vec::new();
    let empty_record = |t| fit::FitRecord {
        seconds_since_unix_epoch: t,
        power: None,
        heart_rate: None,
        cadence: None,
    };

    for x in db.get_session_entries(session_key) {
        if let Ok(((_, d, uuid), v)) = x {
            let seconds_since_unix_epoch = (session_key + d.as_secs()) as u32;
            let mut r = match record {
                Some(mut r) => {
                    if r.seconds_since_unix_epoch == seconds_since_unix_epoch {
                        r
                    } else {
                        if let None = r.power {
                            r.power = Some(last_power);
                        }
                        records.push(r);
                        empty_record(seconds_since_unix_epoch)
                    }
                }
                None => empty_record(seconds_since_unix_epoch),
            };

            record = Some(match uuid {
                UUID::B16(0x2A37) => {
                    r.heart_rate = Some(parse_hrm(&v).bpm as u8);
                    r
                }
                UUID::B16(0x2A63) => {
                    let p = parse_cycling_power_measurement(&v).instantaneous_power as u16;
                    last_power = p;
                    r.power = Some(p);
                    r
                }
                UUID::B16(0x2A5B) => {
                    let csc_measurement = parse_csc_measurement(&v);
                    if let Some(lcm) = last_csc_measurement {
                        let a = lcm.crank.unwrap();
                        let b = csc_measurement.crank.clone().unwrap();
                        if let Some((rpm, _)) = checked_rpm_and_new_count(&a, &b) {
                            r.cadence = Some(rpm as u8);
                        }
                    }
                    last_csc_measurement = Some(csc_measurement);
                    r
                }
                _ => {
                    println!("UUID not matched");
                    r
                }
            });
        }
    }

    fit::to_file(&records)
}
