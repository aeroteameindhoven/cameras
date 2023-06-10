use std::{
    io,
    ops::ControlFlow,
    time::{Duration, Instant},
};

use clap::Parser;
use log::{debug, error, info, trace, warn};
use mavlink::error::MessageReadError;
use serialport::TTYPort;
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

    // TODO:
    // serialport::available_ports();

    info!("connecting to serial port: {:?}", args.port);
    let serial_port = match serialport::new(args.port, 921600)
        .timeout(Duration::from_millis(500))
        .open_native()
    {
        Ok(serial_port) => serial_port,
        Err(error) => {
            error!("failed to open serial port: {error}");
            return;
        }
    };

    info!("initialized, program will gracefully handle errors from now on");

    let mut connection = MavLinkConnection::new(serial_port);

    // Keep trying to handle mavlink commands, handling errors and retrying on timeouts
    loop {
        if let ControlFlow::Break(Err(error)) = handle_mavlink(&mut connection, system_start) {
            todo!("{error}")
        }
    }
}

fn handle_mavlink(
    connection: &mut MavLinkConnection,
    system_start: Instant,
) -> ControlFlow<Result<(), io::Error>, ()> {
    connection.attempt_heartbeat();

    match connection.read_gracefully()? {
        (header, mavlink::common::MavMessage::HEARTBEAT(heartbeat)) => {
            debug!("RX: {header:?} {heartbeat:?}");
        }
        (header, mavlink::common::MavMessage::COMMAND_INT(command)) => {
            let acknowledge = |result| {
                mavlink::common::MavMessage::COMMAND_ACK(mavlink::common::COMMAND_ACK_DATA {
                    command: command.command,
                    result,
                    target_component: header.component_id,
                    target_system: header.system_id,
                    ..Default::default()
                })
            };

            debug!("RX: {header:?} {command:?}");

            match command.command {
                mavlink::common::MavCmd::MAV_CMD_REQUEST_CAMERA_INFORMATION => {
                    connection.write_gracefully(acknowledge(
                        mavlink::common::MavResult::MAV_RESULT_ACCEPTED,
                    ))?;

                    connection.write_gracefully(
                        mavlink::common::MavMessage::CAMERA_INFORMATION(
                            mavlink::common::CAMERA_INFORMATION_DATA {
                                flags:
                                    mavlink::common::CameraCapFlags::CAMERA_CAP_FLAGS_CAPTURE_VIDEO,
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
                            },
                        ),
                    )?;
                }
                mavlink::common::MavCmd::MAV_CMD_CAMERA_STOP_TRACKING => todo!(),
                mavlink::common::MavCmd::MAV_CMD_CAMERA_TRACK_POINT => todo!(),
                mavlink::common::MavCmd::MAV_CMD_CAMERA_TRACK_RECTANGLE => todo!(),
                mavlink::common::MavCmd::MAV_CMD_REQUEST_CAMERA_IMAGE_CAPTURE => todo!(),
                mavlink::common::MavCmd::MAV_CMD_REQUEST_CAMERA_SETTINGS => todo!(),
                mavlink::common::MavCmd::MAV_CMD_RESET_CAMERA_SETTINGS => todo!(),
                mavlink::common::MavCmd::MAV_CMD_SET_CAMERA_FOCUS => todo!(),
                mavlink::common::MavCmd::MAV_CMD_SET_CAMERA_MODE => todo!(),
                mavlink::common::MavCmd::MAV_CMD_SET_CAMERA_ZOOM => todo!(),

                mavlink::common::MavCmd::MAV_CMD_VIDEO_START_CAPTURE => todo!(),
                mavlink::common::MavCmd::MAV_CMD_VIDEO_STOP_CAPTURE => todo!(),
                mavlink::common::MavCmd::MAV_CMD_REQUEST_CAMERA_CAPTURE_STATUS => todo!(),

                _ => {
                    connection.write_gracefully(acknowledge(
                        mavlink::common::MavResult::MAV_RESULT_UNSUPPORTED,
                    ))?;
                }
            }
        }
        _ => (),
    };

    ControlFlow::Continue(())
}

pub struct MavLinkConnection {
    serial_port: TTYPort,
    sequence_number: u8,
    last_heartbeat: Instant,
}

impl MavLinkConnection {
    fn new(serial_port: TTYPort) -> Self {
        Self {
            serial_port,
            sequence_number: 0,
            last_heartbeat: Instant::now(),
        }
    }

    fn attempt_heartbeat(&mut self) -> ControlFlow<Result<(), io::Error>, usize> {
        if self.last_heartbeat.elapsed() >= Duration::from_secs(1) {
            let message = mavlink_heartbeat_message();

            let len = self.write_gracefully(message)?;
            self.last_heartbeat = Instant::now();

            ControlFlow::Continue(len)
        } else {
            ControlFlow::Continue(0)
        }
    }

    fn mavlink_header(&mut self) -> mavlink::MavHeader {
        let header = mavlink::MavHeader {
            system_id: 1,
            component_id: mavlink::common::MavComponent::MAV_COMP_ID_CAMERA as u8,
            sequence: self.sequence_number,
        };

        self.sequence_number = self.sequence_number.wrapping_add(1);

        header
    }

    pub fn write_gracefully(
        &mut self,
        message: mavlink::common::MavMessage,
    ) -> ControlFlow<Result<(), io::Error>, usize> {
        let header = self.mavlink_header();

        match mavlink::write_v2_msg(&mut self.serial_port, header, &message) {
            Ok(length) => {
                debug!("TX[{length}]: {header:?} {message:?}");
                ControlFlow::Continue(length)
            }
            Err(mavlink::error::MessageWriteError::Io(error)) => {
                ControlFlow::Break(match error.kind() {
                    io::ErrorKind::TimedOut => Ok(()),
                    _ => Err(error),
                })
            }
        }
    }

    pub fn read_gracefully(
        &mut self,
    ) -> ControlFlow<Result<(), io::Error>, (mavlink::MavHeader, mavlink::common::MavMessage)> {
        match mavlink::read_v2_msg::<mavlink::common::MavMessage, _>(&mut self.serial_port) {
            Err(MessageReadError::Io(error)) => match error.kind() {
                std::io::ErrorKind::TimedOut => ControlFlow::Break(Ok(())),
                _ => ControlFlow::Break(Err(error)),
            },
            Err(MessageReadError::Parse(error)) => {
                todo!("{error}")
            }
            Ok((header, message)) => ControlFlow::Continue((header, message)),
        }
    }
}

fn mav_str(text: &[u8]) -> [u8; 32] {
    assert!(text.len() <= 32, "text length must be less than 32 bytes");

    let mut buffer = [0u8; 32];

    buffer[0..text.len()].copy_from_slice(text);

    buffer
}

pub fn mavlink_heartbeat_message() -> mavlink::common::MavMessage {
    mavlink::common::MavMessage::HEARTBEAT(mavlink::common::HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: mavlink::common::MavType::MAV_TYPE_CAMERA,
        autopilot: mavlink::common::MavAutopilot::MAV_AUTOPILOT_INVALID,
        base_mode: mavlink::common::MavModeFlag::empty(),
        system_status: mavlink::common::MavState::MAV_STATE_ACTIVE,
        mavlink_version: 0x00,
    })
}
