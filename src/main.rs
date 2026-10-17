// Prevent console window in addition to Slint window in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_camel_case_types)]

use std::error::Error;
use std::sync::mpsc::channel;

use slint::{ComponentHandle, Global, ModelRc, SharedString, VecModel};

slint::include_modules!();

mod logger;
mod protocol;
mod serial;

use logger::LogManager;
use serial::{SerialCmd, get_available_ports, start_serial_worker};

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let logger = LogManager::new();
    let (tx_cmd, rx_cmd) = channel::<SerialCmd>();

    // Initial system greeting in log table
    logger.log(&ui_weak, "SYS", "Garden ESP32 Controller ready.");

    // Start serial worker thread
    start_serial_worker(rx_cmd, ui_weak.clone(), logger.clone());

    let glue = Glue::get(&ui);

    // Callback: refresh_ports() -> [string]
    glue.on_refresh_ports(move || {
        let ports = get_available_ports();
        let slint_ports: Vec<SharedString> = ports.iter().map(|p| p.as_str().into()).collect();
        ModelRc::new(VecModel::from(slint_ports))
    });

    // Callback: connect(port, baud)
    let tx_connect = tx_cmd.clone();
    let logger_connect = logger.clone();
    let ui_weak_connect = ui_weak.clone();
    glue.on_connect(move |port, baud| {
        let port_str = port.to_string();
        let baud_rate = baud.parse::<u32>().unwrap_or(9600);
        if port_str.is_empty() {
            logger_connect.log(&ui_weak_connect, "ERR", "No serial port selected");
            return;
        }
        let _ = tx_connect.send(SerialCmd::Connect {
            port: port_str,
            baud_rate,
        });
    });

    // Callback: disconnect()
    let tx_disconnect = tx_cmd.clone();
    glue.on_disconnect(move || {
        let _ = tx_disconnect.send(SerialCmd::Disconnect);
    });

    // Callback: change_pin(pin, status)
    let tx_pin = tx_cmd.clone();
    let logger_pin = logger.clone();
    let ui_weak_pin = ui_weak.clone();
    glue.on_change_pin(move |pin, status| {
        let command = protocol::format_pin_command(pin, status);
        logger_pin.log(
            &ui_weak_pin,
            "TX",
            &format!("Pin {}: {}", pin, if status { "ON" } else { "OFF" }),
        );
        let _ = tx_pin.send(SerialCmd::Send(command));
    });

    // Callback: send_serial_input(text)
    let tx_input = tx_cmd.clone();
    let logger_input = logger.clone();
    let ui_weak_input = ui_weak.clone();
    glue.on_send_serial_input(move |input| {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            let command = protocol::format_raw_command(trimmed);
            logger_input.log(&ui_weak_input, "TX", trimmed);
            let _ = tx_input.send(SerialCmd::Send(command));
        }
    });

    // Callback: export(logs)
    let logger_export = logger.clone();
    let ui_weak_export = ui_weak.clone();
    glue.on_export(move |_logs| {
        match logger_export.export_logs() {
            Ok(filename) => {
                logger_export.log(
                    &ui_weak_export,
                    "SYS",
                    &format!("Logs exported to {}", filename),
                );
            }
            Err(e) => {
                logger_export.log(
                    &ui_weak_export,
                    "ERR",
                    &format!("Export failed: {}", e),
                );
            }
        }
    });

    ui.run()?;

    Ok(())
}
