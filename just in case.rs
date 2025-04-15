use std::{thread, time::Duration};
use sysinfo::{System, ProcessStatus, Pid};
use users::get_user_by_uid;
use std::io::{self, Write};

fn main() {
    let mut system = System::new_all();
    loop {
        system.refresh_processes();
        system.refresh_memory();
        system.refresh_cpu();
        // Clear terminal

        print!("\x1B[2J\x1B[H"); // Clear screen AND move cursor to top;
        io::stdout().flush().unwrap();
        print!("Pulse – Linux Process Monitor\n");
        println!("{:<6} {:<10} {:>6} {:>6} {:<10} {:<31}",
            "PID", "USER", "CPU%", "MEM%", "STATE", "COMMAND");
        println!("{:-<84}", "");

        let mut processes: Vec<_> = system.processes().values().collect();
        processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());
        for process in processes.iter().take(20) {
            let pid: Pid = process.pid();
            let cpu = process.cpu_usage();
            let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;
            let state = match process.status() {
                ProcessStatus::Run => "Running",
                ProcessStatus::Sleep => "Sleep",
                ProcessStatus::Idle => "Idle",
                ProcessStatus::Stop => "Stopped",
                ProcessStatus::Zombie => "Zombie",
                _ => "Other",
            };
            
            // Fix: Directly dereference the uid to get the u32 value
            let username = process.user_id()
                .and_then(|uid| get_user_by_uid(**uid))
                .map(|u| u.name().to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown".to_string());
            
            let command = process.name();
            let command_display = if command.len() > 29 {
                format!("{}...", &command[..26])
            } else {
                command.to_string()
            };
    
            print!("{:<6} {:<16} {:>6.1} {:>6.1} {:<10} {:<31}\x1B[K\n",
                pid, username, cpu, mem, state, command_display);
        }

        thread::sleep(Duration::from_millis(1000));
    }
        
}

