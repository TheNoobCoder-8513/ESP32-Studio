use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use slint::{Global, Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, Glue, Var_Data};

pub const MAX_SAMPLES: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct VarEntry {
    pub label: String,
    pub max: f32,
    pub history: VecDeque<f32>,
}

#[derive(Clone, Default)]
pub struct TrackManager {
    entries: Arc<Mutex<Vec<VarEntry>>>,
}

thread_local! {
    static UI_VAR_LIST: std::cell::RefCell<Option<std::rc::Rc<VecModel<Var_Data>>>> = const { std::cell::RefCell::new(None) };
    static UI_VAR_SERIES: std::cell::RefCell<Vec<(String, std::rc::Rc<VecModel<f32>>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl TrackManager {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Ingests incoming telemetry JSON and appends the latest values to tracked variables.
    /// Variables are discovered dynamically and appended in stable order.
    pub fn update_telemetry(&self, ui_weak: &slint::Weak<AppWindow>, json_line: &str) {
        let parsed_vars = parse_telemetry_vars(json_line);
        if parsed_vars.is_empty() {
            return;
        }

        let mut list = self.entries.lock().unwrap();

        for (name, val, max_val) in parsed_vars {
            if let Some(entry) = list.iter_mut().find(|e| e.label == name) {
                if max_val > 0.0 {
                    entry.max = max_val;
                }
                if entry.history.len() >= MAX_SAMPLES {
                    entry.history.pop_front();
                }
                entry.history.push_back(val);
            } else {
                let mut history = VecDeque::with_capacity(MAX_SAMPLES);
                history.push_back(val);
                list.push(VarEntry {
                    label: name,
                    max: if max_val > 0.0 { max_val } else { 100.0 },
                    history,
                });
            }
        }

        self.dispatch_to_ui(ui_weak, &list);
    }

    /// Synchronizes the current variables list with the Slint UI model using granular row updates.
    fn dispatch_to_ui(&self, ui_weak: &slint::Weak<AppWindow>, list: &[VarEntry]) {
        // Extract the latest sample and max for each variable
        let snapshot: Vec<(String, f32, f32)> = list
            .iter()
            .map(|e| {
                (
                    e.label.clone(),
                    e.history.back().copied().unwrap_or(0.0),
                    e.max,
                )
            })
            .collect();

        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            UI_VAR_LIST.with(|list_cell| {
                UI_VAR_SERIES.with(|series_cell| {
                    let mut list_opt = list_cell.borrow_mut();
                    let mut series_vec = series_cell.borrow_mut();

                    let var_list = list_opt.get_or_insert_with(|| {
                        let m = std::rc::Rc::new(VecModel::default());
                        Glue::get(&ui).set_variables(ModelRc::new(m.clone()));
                        m
                    });

                    for (label, val, max) in snapshot {
                        if let Some((_, series_model)) = series_vec.iter().find(|(l, _)| l == &label) {
                            if series_model.row_count() >= MAX_SAMPLES {
                                series_model.remove(0);
                            }
                            series_model.push(val);
                        } else {
                            let series_model = std::rc::Rc::new(VecModel::default());
                            series_model.push(val);
                            series_vec.push((label.clone(), series_model.clone()));

                            var_list.push(Var_Data {
                                label: SharedString::from(label),
                                max,
                                data: ModelRc::new(series_model),
                            });
                        }
                    }
                });
            });
        });
    }

    /// Returns the number of currently tracked variables.
    #[allow(dead_code)]
    pub fn tracked_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

/// Parses variable telemetry from JSON.
/// Expects either:
/// - `{"vars": [{"name": "Temperature", "val": 24.5, "max": 100.0}, ...]}`
/// - or with "label" / "value" aliases.
pub fn parse_telemetry_vars(json: &str) -> Vec<(String, f32, f32)> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let vars_arr = match v.get("vars").and_then(|arr| arr.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut results = Vec::new();
    for item in vars_arr {
        let label = item
            .get("name")
            .or_else(|| item.get("label"))
            .and_then(|s| s.as_str());

        let val = item
            .get("val")
            .or_else(|| item.get("value"))
            .and_then(|n| n.as_f64())
            .map(|f| f as f32);

        let max = item
            .get("max")
            .and_then(|n| n.as_f64())
            .map(|f| f as f32)
            .unwrap_or(100.0);

        if let (Some(label), Some(val)) = (label, val) {
            results.push((label.to_string(), val, max));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_telemetry_vars() {
        let json = r#"{"type":"telemetry","vars":[{"name":"Temperature (°C)","val":25.4,"max":100.0},{"name":"Voltage (V)","val":3.28,"max":3.3}]}"#;
        let vars = parse_telemetry_vars(json);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].0, "Temperature (°C)");
        assert!((vars[0].1 - 25.4).abs() < 1e-4);
        assert!((vars[0].2 - 100.0).abs() < 1e-4);
        assert_eq!(vars[1].0, "Voltage (V)");
        assert!((vars[1].1 - 3.28).abs() < 1e-4);
        assert!((vars[1].2 - 3.3).abs() < 1e-4);
    }

    #[test]
    fn test_parse_telemetry_vars_alternate_keys() {
        let json = r#"{"vars":[{"label":"CPU","value":42.0}]}"#;
        let vars = parse_telemetry_vars(json);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].0, "CPU");
        assert_eq!(vars[0].1, 42.0);
        assert_eq!(vars[0].2, 100.0); // default max
    }

    #[test]
    fn test_parse_telemetry_vars_invalid_or_missing() {
        assert!(parse_telemetry_vars("invalid json").is_empty());
        assert!(parse_telemetry_vars(r#"{"type":"ping"}"#).is_empty());
    }
}
