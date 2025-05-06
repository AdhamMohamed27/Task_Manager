

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use sysinfo::{Process, Pid, ProcessStatus, System};
use users::get_user_by_uid;

pub struct CsvExporter;

impl CsvExporter {
    pub fn export_processes(
        processes: &[&Process], 
        system: &System,
        filepath: &str
    ) -> Result<String, String> {
        // Create or open file
        let path = Path::new(filepath);
        let mut file = match File::create(&path) {
            Ok(file) => file,
            Err(e) => return Err(format!("Failed to create CSV file: {}", e)),
        };
        
        // Write header
        if let Err(e) = writeln!(file, "PID,USER,CPU%,MEM%,PRIORITY,FG/BG,STATE,COMMAND") {
            return Err(format!("Failed to write CSV header: {}", e));
        }
        
        // Write process data
        for process in processes {
            let pid: Pid = process.pid();
            let cpu = process.cpu_usage();
            let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;
            
            // Get priority (nice value)
            let stat_path = format!("/proc/{}/stat", pid);
            let stat_content = std::fs::read_to_string(&stat_path).unwrap_or_default();
            let parts: Vec<&str> = stat_content.split_whitespace().collect();
            
            let nice = parts.get(18)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
                
            // Get foreground/background status
            let pgrp = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            let tpgid = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            let fg_bg = if pgrp == tpgid { "FG" } else { "BG" };
            
            // Get process state
            let state = match process.status() {
                ProcessStatus::Run => "Running",
                ProcessStatus::Sleep => "Sleep",
                ProcessStatus::Idle => "Idle",
                ProcessStatus::Stop => "Stopped",
                ProcessStatus::Zombie => "Zombie",
                _ => "Other",
            };
            
            // Get username
            let username = process.user_id()
                .and_then(|uid| get_user_by_uid(**uid))
                .map(|u| u.name().to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".to_string());
                
            // Get command
            let command = process.name();
            
            // Write the line, escaping quotation marks in strings
            if let Err(e) = writeln!(
                file,
                "{},{},{:.1},{:.1},{},{},{},\"{}\"",
                pid,
                username.replace("\"", "\"\""),
                cpu,
                mem,
                nice,
                fg_bg,
                state,
                command.replace("\"", "\"\"")
            ) {
                return Err(format!("Failed to write process data: {}", e));
            }
        }
        
        // Add system summary at the end
        if let Err(e) = writeln!(file, "\nSYSTEM SUMMARY") {
            return Err(format!("Failed to write summary header: {}", e));
        }
        
        let mem_percent = (system.used_memory() as f64 / system.total_memory() as f64) * 100.0;
        let mem_gb = system.total_memory() as f64 / 1_073_741_824.0;
        let mem_used_gb = system.used_memory() as f64 / 1_073_741_824.0;
        let num_cores = system.physical_core_count().unwrap_or(1);
        
        if let Err(e) = writeln!(
            file,
            "Memory Usage,{:.1}GB / {:.1}GB ({:.1}%)",
            mem_used_gb, mem_gb, mem_percent
        ) {
            return Err(format!("Failed to write memory summary: {}", e));
        }
        
        if let Err(e) = writeln!(
            file,
            "CPUs,{} cores",
            num_cores
        ) {
            return Err(format!("Failed to write CPU summary: {}", e));
        }
        
        if let Err(e) = writeln!(
            file,
            "Processes,{}",
            processes.len()
        ) {
            return Err(format!("Failed to write process count: {}", e));
        }
        
        Ok(format!("Process data exported to {}", filepath))
    }
    
    pub fn get_default_filename() -> String {
        use chrono::Local;
        let now = Local::now();
        format!("pulse_export_{}.csv", now.format("%Y%m%d_%H%M%S"))
    }
}