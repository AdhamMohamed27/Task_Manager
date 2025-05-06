// json_export.rs
use serde::Serialize;
use sysinfo::{Process, Pid, ProcessStatus, System};
use users::get_user_by_uid;
use std::{fs::File, io::Write, path::Path};

#[derive(Serialize)]
struct ProcessInfo {
    pid: u32,
    username: String,
    cpu: f32,
    mem: f64,
    nice: i32,
    fg_bg: String,
    state: String,
    command: String,
}

pub struct JsonExporter;

impl JsonExporter {
    pub fn export(processes: &[&Process], system: &System, filepath: &str) -> Result<String, String> {
        let mut data = Vec::new();

        for process in processes {
            let pid = process.pid();
            let cpu = process.cpu_usage();
            let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;

            let stat_path = format!("/proc/{}/stat", pid);
            let stat = std::fs::read_to_string(stat_path).unwrap_or_default();
            let parts: Vec<&str> = stat.split_whitespace().collect();
            let nice = parts.get(18).and_then(|s| s.parse().ok()).unwrap_or(0);
            let pgrp = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let tpgid = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
            let fg_bg = if pgrp == tpgid { "FG" } else { "BG" }.to_string();

            let state = match process.status() {
                ProcessStatus::Run => "Running",
                ProcessStatus::Sleep => "Sleep",
                ProcessStatus::Idle => "Idle",
                ProcessStatus::Stop => "Stopped",
                ProcessStatus::Zombie => "Zombie",
                _ => "Other",
            }.to_string();

            let username = process.user_id()
                .and_then(|uid| get_user_by_uid(**uid))
                .map(|u| u.name().to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".to_string());

            data.push(ProcessInfo {
                pid: pid.as_u32(),
                username,
                cpu,
                mem,
                nice,
                fg_bg,
                state,
                command: process.name().to_string(),
            });
        }

        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        let mut file = File::create(Path::new(filepath)).map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        Ok(format!("Process data exported to {}", filepath))
    }

    pub fn get_default_filename() -> String {
        use chrono::Local;
        let now = Local::now();
        format!("pulse_export_{}.json", now.format("%Y%m%d_%H%M%S"))
    }
}
