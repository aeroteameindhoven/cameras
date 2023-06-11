use std::time::{Instant, SystemTime};

use gpio_cdev::{Chip, EventRequestFlags, LineEvent, LineRequestFlags};
use log::{debug, error, info, trace, warn};
use simplelog::TermLogger;
use systemd_journal_logger::JournalLog;

const GPIO_PIN: u32 = 18;

fn main() {
    let system_start = Instant::now();

    if systemd_journal_logger::connected_to_journal() {
        // If the output streams of this process are directly connected to the
        // systemd journal log directly to the journal to preserve structured
        // log entries (e.g. proper multiline messages, metadata fields, etc.)
        JournalLog::empty()
            .with_syslog_identifier(
                systemd_journal_logger::current_exe_identifier().unwrap_or_default(),
            )
            .install()
            .unwrap();
    } else {
        // Otherwise fall back to logging to standard error.
        TermLogger::init(
            log::LevelFilter::Trace,
            simplelog::ConfigBuilder::new().build(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        )
        .unwrap();
    }

    log::set_max_level(log::LevelFilter::Trace);

    let mut chip = Chip::new("/dev/gpiochip0").expect("gpio chip should be accessible");
    let input = chip.get_line(GPIO_PIN).expect("gpio pin should exist");

    let event_iterator = input
        .events(
            LineRequestFlags::INPUT,
            EventRequestFlags::FALLING_EDGE,
            "px4-camera-trigger-gpio",
        )
        .expect("input events should be subscribable");

    // TODO: start the recording

    info!("initialized, program will gracefully handle errors from now on");

    for event in event_iterator {
        match event {
            Ok(event) => {
                info!("recording requested to stop at {}", event.timestamp());

                // TODO: Stop the recording

                info!(
                    "recording successfully stopped at {}",
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )
            }
            Err(error) => {
                error!("{error}");
                warn!("encountered error reading event from event iterator, skipping...");
            }
        }
    }
}
