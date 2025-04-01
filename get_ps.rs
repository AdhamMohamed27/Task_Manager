use std::{thread, time::Duration};
use sysinfo::{ProcessStatus, System};

fn main() {
    let mut s = System::new_all();

    loop {
        s.refresh_processes(); // Refresh process states

        println!("Listing all processes with their states:");

        for (pid, process) in s.processes() {
            let state = match process.status() {
                ProcessStatus::Run => "Running".to_string(),
                ProcessStatus::Idle => "Idle".to_string(),
                ProcessStatus::Sleep => "Sleeping".to_string(),
                ProcessStatus::Stop => "Stopped".to_string(),
                ProcessStatus::Zombie => "Zombie".to_string(),
                ProcessStatus::Tracing => "Tracing".to_string(),
                ProcessStatus::Dead => "Dead".to_string(),
                ProcessStatus::Wakekill => "Wakekill".to_string(),
                ProcessStatus::Waking => "Waking".to_string(),
                ProcessStatus::LockBlocked => "Lock Blocked".to_string(),
                ProcessStatus::Unknown(code) => format!("Unknown ({})", code),
                _ => "Other".to_string(), // Catch-all case
            };

            println!(
                "PID: {} | Name: {} | State: {} | CPU Usage: {:.2}%",
                pid,
                process.name(),
                state,
                process.cpu_usage()
            );
        }

        println!("----------------------");

        thread::sleep(Duration::from_secs(3)); // Sleep to reduce CPU usage
    }
}
