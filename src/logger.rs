use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use chrono::Local;
use slint::{Global, Model, ModelRc, VecModel};

use crate::{AppWindow, Glue, Log_Data, Serial_Data};

pub const MAX_LOG_LINES: usize = 180;
pub const MAX_SERIAL_LINES: usize = 60;

thread_local! {
    static UI_LOG_MODEL: RefCell<Option<Rc<VecModel<Log_Data>>>> = const { RefCell::new(None) };
    static UI_SERIAL_MODEL: RefCell<Option<Rc<VecModel<Serial_Data>>>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct LogManager {
    storage: Arc<Mutex<VecDeque<Log_Data>>>,
    serial_storage: Arc<Mutex<VecDeque<Serial_Data>>>,
}

impl LogManager {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))),
            serial_storage: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_SERIAL_LINES))),
        }
    }

    /// Appends a new formatted log entry and dispatches granular row updates to the Slint UI thread.
    /// Updates general logs (limit 180). Serial table (limit 60) ONLY receives serial traffic (no telemetry).
    pub fn log(&self, ui_weak: &slint::Weak<AppWindow>, source: &str, msg: &str) {
        let now = Local::now();
        let date_str = now.format("%Y-%m-%d").to_string();
        let time_str = now.format("%H:%M:%S").to_string();
        let formatted_msg = format!("[{}] {}", source, msg);
        let is_telemetry = source == "TEL" || msg.contains(r#""type":"telemetry""#);

        let item = Log_Data {
            date: date_str.clone().into(),
            time: time_str.clone().into(),
            outflow: source == "TX",
            is_tele: is_telemetry,
            log: formatted_msg.clone().into(),
        };

        let mut storage = self.storage.lock().unwrap();
        if storage.len() >= MAX_LOG_LINES {
            storage.pop_front();
        }
        storage.push_back(item.clone());

        // Serial table should ONLY show serial outputs, strictly NO telemetry
        let serial_item = if !is_telemetry {
            let s_item = Serial_Data {
                date: date_str.into(),
                time: time_str.into(),
                log: formatted_msg.into(),
            };

            let mut serial_storage = self.serial_storage.lock().unwrap();
            if serial_storage.len() >= MAX_SERIAL_LINES {
                serial_storage.pop_front();
            }
            serial_storage.push_back(s_item.clone());
            Some(s_item)
        } else {
            None
        };

        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            // 1. Granular update for General Logs (limit 180)
            UI_LOG_MODEL.with(|cell| {
                let mut opt = cell.borrow_mut();
                let model = opt.get_or_insert_with(|| {
                    let m = Rc::new(VecModel::default());
                    Glue::get(&ui).set_logs(ModelRc::new(m.clone()));
                    m
                });

                if model.row_count() >= MAX_LOG_LINES {
                    model.remove(0);
                }
                model.push(item);
            });

            // 2. Granular update for Serial Tracking (limit 60) - ONLY if not telemetry
            if let Some(s_item) = serial_item {
                UI_SERIAL_MODEL.with(|cell| {
                    let mut opt = cell.borrow_mut();
                    let model = opt.get_or_insert_with(|| {
                        let m = Rc::new(VecModel::default());
                        Glue::get(&ui).set_serial_output(ModelRc::new(m.clone()));
                        m
                    });

                    if model.row_count() >= MAX_SERIAL_LINES {
                        model.remove(0);
                    }
                    model.push(s_item);
                });
            }
        });
    }

    /// Clears the in-memory general logs buffer and updates the Slint UI.
    pub fn clear(&self, ui_weak: &slint::Weak<AppWindow>) {
        let mut storage = self.storage.lock().unwrap();
        storage.clear();

        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            UI_LOG_MODEL.with(|cell| {
                let m = Rc::new(VecModel::default());
                Glue::get(&ui).set_logs(ModelRc::new(m.clone()));
                *cell.borrow_mut() = Some(m);
            });
        });
    }

    /// Clears the in-memory serial logs buffer and updates the Slint UI.
    pub fn clear_serial(&self, ui_weak: &slint::Weak<AppWindow>) {
        let mut storage = self.serial_storage.lock().unwrap();
        storage.clear();

        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            UI_SERIAL_MODEL.with(|cell| {
                let m = Rc::new(VecModel::default());
                Glue::get(&ui).set_serial_output(ModelRc::new(m.clone()));
                *cell.borrow_mut() = Some(m);
            });
        });
    }

    /// Exports all log items to a formatted text file in the working directory.
    #[allow(dead_code)]
    pub fn export_logs(&self) -> Result<String, std::io::Error> {
        let storage = self.storage.lock().unwrap();
        let now = Local::now();
        let filename = format!("garden_logs_{}.txt", now.format("%Y%m%d_%H%M%S"));

        let mut file = File::create(&filename)?;
        for item in storage.iter() {
            writeln!(file, "{} {} {}", item.date, item.time, item.log)?;
        }

        Ok(filename)
    }

    /// Exports serial log items to a formatted text file in the working directory.
    pub fn export_serial_logs(&self) -> Result<String, std::io::Error> {
        let storage = self.serial_storage.lock().unwrap();
        let now = Local::now();
        let filename = format!("garden_serial_{}.txt", now.format("%Y%m%d_%H%M%S"));

        let mut file = File::create(&filename)?;
        for item in storage.iter() {
            writeln!(file, "{} {} {}", item.date, item.time, item.log)?;
        }

        Ok(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_constants() {
        assert_eq!(MAX_LOG_LINES, 180);
        assert_eq!(MAX_SERIAL_LINES, 60);
    }
}
