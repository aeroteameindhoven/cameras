use std::path::PathBuf;

use clap::Parser;
use log::{debug, error, info, trace, warn};
use mavlink::{error::MessageReadError, MavHeader};
use simplelog::TermLogger;
use systemd_journal_logger::JournalLog;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// Listen to MavLink commands over a serial port to start and stop recording of 4 video streams
struct Arguments {
    #[arg(default_value = "/dev/ttyUSB0")]
    /// serial port used to receive MavLink commands
    port: PathBuf,
}

fn main() {
    let args = Arguments::parse();

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

    info!("connecting to serial port: {:?}", args.port);
    let mut serial_port = match serial2::SerialPort::open(args.port, 115200) {
        Ok(serial_port) => serial_port,
        Err(error) => {
            error!("failed to open serial port: {error}");
            return;
        }
    };

    info!("initialized, program will gracefully handle errors from now on");

    match mavlink::read_v2_msg::<mavlink::cubepilot::MavMessage, _>(&mut serial_port) {
        Ok((
            MavHeader {
                system_id,
                component_id,
                sequence,
            },
            message,
        )) => debug!("RX[{system_id}:{component_id}:{sequence}]: {message:?}"),
        Err(MessageReadError::Io(error)) => todo!(),
        Err(MessageReadError::Parse(error)) => todo!(),
    };
}
