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
pub mod simulator;

use logger::LogManager;
use serial::{SerialCmd, get_available_ports, start_serial_worker};
use simulator::start_esp_simulator;

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let logger = LogManager::new();
    let (tx_cmd, rx_cmd) = channel::<SerialCmd>();

    // Initial system greeting in log table
    logger.log(&ui_weak, "SYS", "Garden ESP32 Controller ready (Simulator Active).");

    // 1. Start ESP32 Thread Simulator (sends telemetry data every 1 second)
    let (tx_to_main, rx_from_sim) = channel::<String>();
    let sim_handle = start_esp_simulator(tx_to_main);

    // Forward simulated telemetry packets to the UI logger
    let ui_weak_sim = ui_weak.clone();
    let logger_sim = logger.clone();
    std::thread::spawn(move || {
        while let Ok(data) = rx_from_sim.recv() {
            logger_sim.log(&ui_weak_sim, "TEL", &data);
        }
    });

    // Start serial worker thread for physical ports
    start_serial_worker(rx_cmd, ui_weak.clone(), logger.clone());

    let glue = Glue::get(&ui);

    // Mark connected by default to virtual simulator
    glue.set_app_connected(true);

    // Callback: refresh_ports() -> [string]
    glue.on_refresh_ports(move || {
        let mut ports = get_available_ports();
        ports.insert(0, "ESP32-Simulator (Virtual)".to_string());
        let slint_ports: Vec<SharedString> = ports.iter().map(|p| p.as_str().into()).collect();
        ModelRc::new(VecModel::from(slint_ports))
    });

    // Callback: connect(port, baud)
    let tx_connect = tx_cmd.clone();
    let logger_connect = logger.clone();
    let ui_weak_connect = ui_weak.clone();
    glue.on_connect(move |port, baud| {
        let port_str = port.to_string();
        let baud_rate = baud.parse::<u32>().unwrap_or(115200);
        if port_str.is_empty() {
            logger_connect.log(&ui_weak_connect, "ERR", "No serial port selected");
            return;
        }
        if port_str.contains("Simulator") || port_str.contains("Virtual") {
            logger_connect.log(&ui_weak_connect, "SYS", "Connected to Virtual ESP32 Simulator");
            let _ = ui_weak_connect.upgrade_in_event_loop(|ui| {
                Glue::get(&ui).set_app_connected(true);
            });
            return;
        }
        let _ = tx_connect.send(SerialCmd::Connect {
            port: port_str,
            baud_rate,
        });
    });

    // Callback: disconnect()
    let tx_disconnect = tx_cmd.clone();
    let logger_disconnect = logger.clone();
    let ui_weak_disconnect = ui_weak.clone();
    glue.on_disconnect(move || {
        logger_disconnect.log(&ui_weak_disconnect, "SYS", "Disconnected from ESP32");
        let _ = tx_disconnect.send(SerialCmd::Disconnect);
        let _ = ui_weak_disconnect.upgrade_in_event_loop(|ui| {
            Glue::get(&ui).set_app_connected(false);
        });
    });

    // Callback: change_pin(pin, status)
    let tx_pin = tx_cmd.clone();
    let sim_cmd_pin = sim_handle.cmd_sender.clone();
    let logger_pin = logger.clone();
    let ui_weak_pin = ui_weak.clone();
    glue.on_change_pin(move |pin, status| {
        let command = protocol::format_pin_command(pin, status);
        logger_pin.log(
            &ui_weak_pin,
            "TX",
            &format!("Pin {}: {}", pin, if status { "ON" } else { "OFF" }),
        );
        // Send command to both simulator and serial port
        let _ = sim_cmd_pin.send(command.clone());
        let _ = tx_pin.send(SerialCmd::Send(command));
    });

    // Callback: send_serial_input(text)
    let tx_input = tx_cmd.clone();
    let sim_cmd_input = sim_handle.cmd_sender.clone();
    let logger_input = logger.clone();
    let ui_weak_input = ui_weak.clone();
    glue.on_send_serial_input(move |input| {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            let command = protocol::format_raw_command(trimmed);
            logger_input.log(&ui_weak_input, "TX", trimmed);
            // Send command to both simulator and serial port
            let _ = sim_cmd_input.send(command.clone());
            let _ = tx_input.send(SerialCmd::Send(command));
        }
    });

    // Callback: toggle_pin_track(gpio, active)
    let logger_track = logger.clone();
    let ui_weak_track = ui_weak.clone();
    glue.on_toggle_pin_track(move |gpio, active| {
        logger_track.log(
            &ui_weak_track,
            "SYS",
            &format!("Tracking {} for GPIO {}", if active { "ENABLED" } else { "DISABLED" }, gpio),
        );
    });

    // Callback: export_serial_output(logs)
    let logger_export = logger.clone();
    let ui_weak_export = ui_weak.clone();
    glue.on_export_serial_output(move |_logs| {
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
