use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use chrono::Local;
use slint::{Global, ModelRc, VecModel};

use crate::{AppWindow, Glue, LogItem_Data};

pub const MAX_LOG_LINES: usize = 500;

#[derive(Clone)]
pub struct LogManager {
    storage: Arc<Mutex<VecDeque<LogItem_Data>>>,
}

impl LogManager {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))),
        }
    }

    /// Appends a new formatted log entry and dispatches the update to the Slint UI thread.
    pub fn log(&self, ui_weak: &slint::Weak<AppWindow>, source: &str, msg: &str) {
        let now = Local::now();
        let date_str = now.format("%Y-%m-%d").to_string();
        let time_str = now.format("%H:%M:%S").to_string();
        let formatted_msg = format!("[{}] {}", source, msg);

        let item = LogItem_Data {
            date: date_str.into(),
            time: time_str.into(),
            log: formatted_msg.into(),
        };

        let mut storage = self.storage.lock().unwrap();
        if storage.len() >= MAX_LOG_LINES {
            storage.pop_front();
        }
        storage.push_back(item);

        let snapshot: Vec<LogItem_Data> = storage.iter().cloned().collect();
        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let glue = Glue::get(&ui);
            glue.set_logs(ModelRc::new(VecModel::from(snapshot)));
        });
    }

    /// Clears the in-memory log buffer and updates the Slint UI.
    #[allow(dead_code)]
    pub fn clear(&self, ui_weak: &slint::Weak<AppWindow>) {
        let mut storage = self.storage.lock().unwrap();
        storage.clear();

        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let glue = Glue::get(&ui);
            glue.set_logs(ModelRc::new(VecModel::default()));
        });
    }

    /// Exports all log items to a formatted text file in the working directory.
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
}
