use std::sync::mpsc::{channel, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;

/// Represents one physical pin on the ESP32 NodeMCU DevKit v1 (30-pin version).
#[derive(Debug, Clone)]
pub struct SimulatedPin {
    pub pin_number: usize, // 1 through 30
    pub name: &'static str,
    pub gpio: i32,         // -1 for power/ground/reset lines (EN, GND, VIN, 3V3)
    pub is_controllable: bool,
    pub status: bool,      // true = HIGH (3.3V), false = LOW (0V)
}

/// NodeMCU ESP-WROOM-32 DevKit v1 30-Pin definitions in physical pin order.
pub fn get_nodemcu_30_pins() -> Vec<SimulatedPin> {
    vec![
        // Left Header (Pins 1 - 15)
        SimulatedPin { pin_number: 1,  name: "EN",        gpio: -1, is_controllable: false, status: true },
        SimulatedPin { pin_number: 2,  name: "GPIO36",    gpio: 36, is_controllable: false, status: false }, // SensVP
        SimulatedPin { pin_number: 3,  name: "GPIO39",    gpio: 39, is_controllable: false, status: false }, // SensVN
        SimulatedPin { pin_number: 4,  name: "GPIO34",    gpio: 34, is_controllable: false, status: false },
        SimulatedPin { pin_number: 5,  name: "GPIO35",    gpio: 35, is_controllable: false, status: false },
        SimulatedPin { pin_number: 6,  name: "GPIO32",    gpio: 32, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 7,  name: "GPIO33",    gpio: 33, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 8,  name: "GPIO25",    gpio: 25, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 9,  name: "GPIO26",    gpio: 26, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 10, name: "GPIO27",    gpio: 27, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 11, name: "GPIO14",    gpio: 14, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 12, name: "GPIO12",    gpio: 12, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 13, name: "GPIO13",    gpio: 13, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 14, name: "GND",       gpio: -1, is_controllable: false, status: false },
        SimulatedPin { pin_number: 15, name: "VIN",       gpio: -1, is_controllable: false, status: true },

        // Right Header (Pins 16 - 30)
        SimulatedPin { pin_number: 16, name: "GPIO23",    gpio: 23, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 17, name: "GPIO22",    gpio: 22, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 18, name: "GPIO1",     gpio: 1,  is_controllable: true,  status: false }, // TX0
        SimulatedPin { pin_number: 19, name: "GPIO3",     gpio: 3,  is_controllable: true,  status: false }, // RX0
        SimulatedPin { pin_number: 20, name: "GPIO21",    gpio: 21, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 21, name: "GPIO19",    gpio: 19, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 22, name: "GPIO18",    gpio: 18, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 23, name: "GPIO5",     gpio: 5,  is_controllable: true,  status: false },
        SimulatedPin { pin_number: 24, name: "GPIO17",    gpio: 17, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 25, name: "GPIO16",    gpio: 16, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 26, name: "GPIO4",     gpio: 4,  is_controllable: true,  status: false },
        SimulatedPin { pin_number: 27, name: "GPIO2",     gpio: 2,  is_controllable: true,  status: false }, // Onboard LED
        SimulatedPin { pin_number: 28, name: "GPIO15",    gpio: 15, is_controllable: true,  status: false },
        SimulatedPin { pin_number: 29, name: "GND",       gpio: -1, is_controllable: false, status: false },
        SimulatedPin { pin_number: 30, name: "3V3",       gpio: -1, is_controllable: false, status: true },
    ]
}

/// Handle returned to the main program to interact with the running ESP simulator.
pub struct EspSimulatorHandle {
    /// Channel to send incoming serial commands to the ESP simulator (e.g. "23:on\n", "2:off\n")
    pub cmd_sender: Sender<String>,
    running: Arc<AtomicBool>,
}

impl EspSimulatorHandle {
    /// Stops the simulator background thread.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Spawns the ESP simulator thread.
/// 
/// - `tx_to_main`: Channel through which formatted JSON data is sent to the main application every second.
/// - Returns: `(EspSimulatorHandle, Receiver<String>)` or takes a custom sender to bridge with the host.
pub fn start_esp_simulator(tx_to_main: Sender<String>) -> EspSimulatorHandle {
    let (tx_cmd, rx_cmd) = channel::<String>();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    thread::spawn(move || {
        let mut pins = get_nodemcu_30_pins();
        let start_time = Instant::now();
        let mut tick_counter: u64 = 0;

        println!("ESP32 Simulator thread started (NodeMCU DevKit v1 - 30 pins)");

        while running_clone.load(Ordering::SeqCst) {
            let loop_start = Instant::now();

            // 1. Process any incoming commands from the main program ("<pin>:on", "<pin>:off")
            while let Ok(cmd) = rx_cmd.try_recv() {
                let trimmed = cmd.trim();
                if let Some((pin_str, action)) = trimmed.split_once(':') {
                    if let Ok(gpio_num) = pin_str.parse::<i32>() {
                        let new_state = action.eq_ignore_ascii_case("on") || action == "1";
                        let mut found = false;
                        for pin in pins.iter_mut() {
                            if pin.gpio == gpio_num && pin.is_controllable {
                                pin.status = new_state;
                                found = true;
                                break;
                            }
                        }
                        if found {
                            let time_str = Local::now().format("%H:%M:%S").to_string();
                            println!("ESP : {} : [ACK] Pin {} set to {}", time_str, gpio_num, if new_state { "ON" } else { "OFF" });
                        }
                    }
                }
            }

            // 2. Simulate slight dynamic activity (e.g., toggle onboard LED on GPIO 2 every 2s, wiggle touch pin GPIO 4)
            tick_counter += 1;
            if tick_counter % 2 == 0 {
                if let Some(led) = pins.iter_mut().find(|p| p.gpio == 2) {
                    led.status = !led.status;
                }
            }

            // 3. Format telemetry data matching flash.cpp JSON schema
            let uptime_s = start_time.elapsed().as_secs();
            let mut pins_json = String::from("[");
            for (i, pin) in pins.iter().enumerate() {
                pins_json.push_str(&format!(
                    r#"{{"pin":{},"name":"{}","gpio":{},"status":{}}}"#,
                    pin.pin_number,
                    pin.name,
                    pin.gpio,
                    if pin.status { "true" } else { "false" }
                ));
                if i < pins.len() - 1 {
                    pins_json.push(',');
                }
            }
            pins_json.push(']');

            // Simulate realistic environmental and system variables
            let temp = 24.5 + ((tick_counter as f32 * 0.3).sin() * 2.5);
            let volt = 3.28 + ((tick_counter as f32 * 0.5).sin() * 0.03);
            let humidity = 50.0 + ((tick_counter as f32 * 0.2).cos() * 5.0);
            let cpu = 15.0 + ((tick_counter as f32 * 0.7).sin().abs() * 25.0);
            let heap = 215.0 + ((tick_counter as f32 * 0.1).sin() * 15.0);

            let vars_json = format!(
                r#"[{{"name":"Temperature (°C)","val":{:.1},"max":100.0}},{{"name":"Voltage (V)","val":{:.2},"max":3.3}},{{"name":"Humidity (%)","val":{:.1},"max":100.0}},{{"name":"CPU Load (%)","val":{:.1},"max":100.0}},{{"name":"Free Heap (KB)","val":{:.1},"max":320.0}}]"#,
                temp, volt, humidity, cpu, heap
            );

            let data_payload = format!(
                r#"{{"type":"telemetry","device":"ESP32-NodeMCU-v1","uptime_s":{},"pins":{},"vars":{}}}"#,
                uptime_s, pins_json, vars_json
            );

            // 4. Print to console in the required format:
            // ESP : Time : {Data here}
            let time_str = Local::now().format("%H:%M:%S").to_string();
            println!("ESP : {} : {}", time_str, data_payload);

            // 5. Send data payload to the main program
            let _ = tx_to_main.send(data_payload);

            // 6. Sleep for the remainder of the 2-second interval (30 packets / min)
            let elapsed = loop_start.elapsed();
            if elapsed < Duration::from_secs(2) {
                thread::sleep(Duration::from_secs(2) - elapsed);
            }
        }

        println!("ESP32 Simulator thread stopped.");
    });

    EspSimulatorHandle {
        cmd_sender: tx_cmd,
        running,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nodemcu_30_pins_count() {
        let pins = get_nodemcu_30_pins();
        assert_eq!(pins.len(), 30);
        assert_eq!(pins[0].name, "EN");
        assert_eq!(pins[13].name, "GND");
        assert_eq!(pins[14].name, "VIN");
        assert_eq!(pins[28].name, "GND");
        assert_eq!(pins[29].name, "3V3");
    }

    #[test]
    fn test_simulator_sends_data_and_handles_commands() {
        let (tx_to_main, rx_from_sim) = channel::<String>();
        let handle = start_esp_simulator(tx_to_main);

        // Receive at least one telemetry packet
        let first_msg = rx_from_sim
            .recv_timeout(Duration::from_millis(2500))
            .expect("Should receive data within 2.5s");
        assert!(first_msg.contains("ESP32-NodeMCU-v1"));
        assert!(first_msg.contains(r#""pin":1"#));
        assert!(first_msg.contains(r#""vars""#));
        assert!(first_msg.contains("Temperature"));

        // Send a command to toggle GPIO 23 ON
        handle.cmd_sender.send("23:on\n".to_string()).unwrap();

        // Receive next packet with updated GPIO 23
        let second_msg = rx_from_sim
            .recv_timeout(Duration::from_millis(2500))
            .expect("Should receive second packet");
        assert!(second_msg.contains(r#"{"pin":16,"name":"GPIO23","gpio":23,"status":true}"#));

        handle.stop();
    }
}
