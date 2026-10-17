use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

use slint::Global;

use crate::logger::LogManager;
use crate::{AppWindow, Glue};

#[derive(Debug)]
pub enum SerialCmd {
    Connect { port: String, baud_rate: u32 },
    Disconnect,
    Send(String),
}

/// Enumerates currently available serial ports on the operating system.
pub fn get_available_ports() -> Vec<String> {
    match serialport::available_ports() {
        Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
        Err(_) => Vec::new(),
    }
}

/// Starts the dedicated serial worker thread that executes I/O operations and dispatches events.
pub fn start_serial_worker(
    rx_cmd: Receiver<SerialCmd>,
    ui_weak: slint::Weak<AppWindow>,
    logger: LogManager,
) {
    thread::spawn(move || {
        let mut port: Option<Box<dyn serialport::SerialPort>> = None;
        let mut current_port_name = String::new();
        let mut rx_buffer = [0u8; 1024];
        let mut line_accumulator = String::new();

        loop {
            // Process all pending commands from UI
            while let Ok(cmd) = rx_cmd.try_recv() {
                match cmd {
                    SerialCmd::Connect { port: port_name, baud_rate } => {
                        // Close any existing connection
                        if port.is_some() {
                            port = None;
                        }

                        match serialport::new(&port_name, baud_rate)
                            .timeout(Duration::from_millis(50))
                            .open()
                        {
                            Ok(mut p) => {
                                // Clear RTS & DTR so the ESP32 is not held in hardware reset on Linux
                                let _ = p.write_data_terminal_ready(false);
                                let _ = p.write_request_to_send(false);
                                let _ = p.clear(serialport::ClearBuffer::All);

                                current_port_name = port_name.clone();
                                port = Some(p);

                                logger.log(
                                    &ui_weak,
                                    "SYS",
                                    &format!("Connected to {} @ {} baud", port_name, baud_rate),
                                );

                                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                    Glue::get(&ui).set_app_connected(true);
                                });
                            }
                            Err(e) => {
                                logger.log(
                                    &ui_weak,
                                    "ERR",
                                    &format!("Failed to open {}: {}", port_name, e),
                                );

                                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                    Glue::get(&ui).set_app_connected(false);
                                });
                            }
                        }
                    }
                    SerialCmd::Disconnect => {
                        if port.is_some() {
                            port = None;
                            logger.log(
                                &ui_weak,
                                "SYS",
                                &format!("Disconnected from {}", current_port_name),
                            );
                            current_port_name.clear();
                        }

                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            Glue::get(&ui).set_app_connected(false);
                        });
                    }
                    SerialCmd::Send(data) => {
                        if let Some(ref mut p) = port {
                            if let Err(e) = p.write_all(data.as_bytes()) {
                                logger.log(&ui_weak, "ERR", &format!("Write error: {}", e));
                            } else {
                                let _ = p.flush();
                            }
                        } else {
                            logger.log(&ui_weak, "ERR", "Not connected to any serial port");
                        }
                    }
                }
            }

            // Non-blocking read incoming serial bytes
            if let Some(ref mut p) = port {
                match p.read(&mut rx_buffer) {
                    Ok(bytes_read) if bytes_read > 0 => {
                        let text = String::from_utf8_lossy(&rx_buffer[..bytes_read]);
                        line_accumulator.push_str(&text);

                        while let Some(pos) = line_accumulator.find('\n') {
                            let line = line_accumulator[..pos].trim_matches('\r').to_string();
                            line_accumulator.drain(..=pos);
                            if !line.is_empty() {
                                logger.log(&ui_weak, "RX", &line);
                            }
                        }
                    }
                    Ok(_) => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(ref e)
                        if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        logger.log(&ui_weak, "ERR", &format!("Serial read error: {}", e));
                        port = None;
                        current_port_name.clear();
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            Glue::get(&ui).set_app_connected(false);
                        });
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(50));
            }
        }
    });
}
