use std::time::{Instant, SystemTime};

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
    port: String,
}

fn main() {
    let system_start = Instant::now();

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

    let camera_manager =
        libcamera::camera_manager::CameraManager::new().expect("failed to initialize libcamera");

    let camera_list = camera_manager.cameras();

    for i in 0..camera_list.len() {
        let camera = camera_list.get(i).unwrap();

        let properties = camera.properties();

        let location: libcamera::properties::Location = properties.get().unwrap();
        let model: libcamera::properties::Model = properties.get().unwrap();
        let properties: libcamera::properties::PixelArraySize = properties.get().unwrap();

        debug!(
            "{:?} {} @ {}x{}",
            location, model.0, properties.0.width, properties.0.height
        );
    }

    // TODO:
    // serialport::available_ports();

    info!("connecting to serial port: {:?}", args.port);
    let mut serial_port = match serialport::new(args.port, 115200).open_native() {
        Ok(serial_port) => serial_port,
        Err(error) => {
            error!("failed to open serial port: {error}");
            return;
        }
    };

    info!("initialized, program will gracefully handle errors from now on");

    let mut mavlink_header = {
        let mut sequence = 0;
        move || {
            let header = mavlink::MavHeader {
                system_id: 1,
                component_id: mavlink::common::MavComponent::MAV_COMP_ID_CAMERA as u8,
                sequence,
            };

            sequence += 1;

            header
        }
    };

    let header = mavlink_header();
    let message = mavlink_heartbeat_message();
    match mavlink::write_v2_msg(&mut serial_port, header, &message) {
        Ok(length) => {
            debug!("TX[{length}]: {header:?} {message:?}");
        }
        Err(mavlink::error::MessageWriteError::Io(error)) => todo!(),
    };

    match mavlink::read_v2_msg::<mavlink::common::MavMessage, _>(&mut serial_port) {
        Err(MessageReadError::Io(error)) => todo!(),
        Err(MessageReadError::Parse(error)) => todo!(),
        Ok((header, message)) => {
            debug!("RX: {header:?} {message:?}");

            match message {
                mavlink::common::MavMessage::COMMAND_INT(command) => {
                    {
                        let header = mavlink_header();
                        let message = mavlink::common::MavMessage::COMMAND_ACK(
                            mavlink::common::COMMAND_ACK_DATA {
                                command: command.command,
                                result: mavlink::common::MavResult::MAV_RESULT_ACCEPTED,
                                ..Default::default()
                            },
                        );
                        match mavlink::write_v2_msg(&mut serial_port, header, &message) {
                            Ok(length) => {
                                debug!("TX[{length}]: {header:?} {message:?}");
                            }
                            Err(mavlink::error::MessageWriteError::Io(error)) => todo!(),
                        };
                    }

                    fn mav_str(text: &[u8]) -> [u8; 32] {
                        assert!(text.len() <= 32, "text length must be less than 32 bytes");

                        let mut buffer = [0u8; 32];

                        buffer[0..text.len()].copy_from_slice(text);

                        buffer
                    }

                    {
                        let header = mavlink_header();
                        let message = mavlink::common::CAMERA_INFORMATION_DATA {
                            flags: mavlink::common::CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_VIDEO,
                            focal_length: 5.1,
                            lens_id: 0,
                            resolution_h: todo!(),
                            resolution_v: todo!(),
                            sensor_size_h: todo!(),
                            sensor_size_v: todo!(),
                            time_boot_ms: system_start.elapsed().as_millis() as u32,
                            firmware_version: todo!(),
                            model_name: mav_str(b"16MP IMX519 Quad-Camera Kit"),
                            vendor_name: mav_str(b"arducam"),
                            ..Default::default()
                        };
                        match mavlink::write_v2_msg(
                            &mut serial_port,
                            header,
                            &mavlink::common::MavMessage::CAMERA_INFORMATION(message),
                        ) {
                            Ok(length) => {
                                debug!("TX[{length}]: {header:?} {message:?}");
                            }
                            Err(mavlink::error::MessageWriteError::Io(error)) => todo!(),
                        };
                    }
                }
                _ => todo!(),
            }
        }
    };
}

pub fn mavlink_heartbeat_message() -> mavlink::common::MavMessage {
    mavlink::common::MavMessage::HEARTBEAT(mavlink::common::HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: mavlink::common::MavType::MAV_TYPE_CAMERA,
        autopilot: mavlink::common::MavAutopilot::MAV_AUTOPILOT_INVALID,
        base_mode: mavlink::common::MavModeFlag::empty(),
        system_status: mavlink::common::MavState::MAV_STATE_STANDBY,
        mavlink_version: 0x00,
    })
}
