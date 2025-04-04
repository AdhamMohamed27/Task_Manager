use std::{env, thread, time::Duration};
use sysinfo::{ProcessExt, ProcessStatus, System, SystemExt, UserExt};
use chrono::{DateTime, Local};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut continuous = false;
    let mut interval = 3;
    let mut show_header = true;
    
    // Simple argument parsing
    for (i, arg) in args.iter().enumerate() {
        if arg == "--continuous" || arg == "-c" {
            continuous = true;
            if i + 1 < args.len() {
                if let Ok(seconds) = args[i + 1].parse::<u64>() {
                    interval = seconds;
                }
            }
        }
        if arg == "--no-header" || arg == "-n" {
            show_header = false;
        }
    }

    let mut s = System::new_all();
    
    loop {
        // Clear screen in continuous mode
        if continuous {
            print!("\x1B[2J\x1B[1;1H");  // ANSI escape code to clear screen
        }
        
        s.refresh_all();
        
        if show_header {
            println!("{:<8} {:<8} {:<8} {:<10} {:<8} {:<12} {:<20}",
                "PID", "USER", "CPU%", "MEM (MB)", "STATE", "START", "COMMAND");
            println!("{:-<80}", "");
        }

        // Get all processes and sort by PID
        let mut processes: Vec<_> = s.processes().iter().collect();
        processes.sort_by_key(|&(pid, _)| *pid);

        for (&pid, process) in processes {
            // Get process owner
            let user = s.get_user_by_id(process.user_id())
                .map_or_else(|| "unknown".to_string(), |user| user.name().to_string());
            
            // Format process state
            let state = match process.status() {
                ProcessStatus::Run => "Running",
                ProcessStatus::Idle => "Idle",
                ProcessStatus::Sleep => "Sleep",
                ProcessStatus::Stop => "Stopped",
                ProcessStatus::Zombie => "Zombie",
                ProcessStatus::Tracing => "Tracing",
                ProcessStatus::Dead => "Dead",
                ProcessStatus::Wakekill => "Wakekill",
                ProcessStatus::Waking => "Waking",
                ProcessStatus::LockBlocked => "Locked",
                ProcessStatus::Unknown(code) => return format!("Unknown({})", code),
                _ => "Other",
            };
            
            // Format memory usage in MB
            let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
            
            // Format start time
            let start_time = DateTime::<Local>::from(process.start_time());
            let formatted_time = if start_time.date_naive() == Local::now().date_naive() {
                start_time.format("%H:%M:%S").to_string()
            } else {
                start_time.format("%b %d %H:%M").to_string()
            };
            
            // Get command with arguments
            let cmd = process.cmd().join(" ");
            let display_cmd = if cmd.is_empty() { process.name().to_string() } else { cmd };
            
            println!(
                "{:<8} {:<8} {:<8.1} {:<10.1} {:<8} {:<12} {:<20}",
                pid.to_string(),
                user,
                process.cpu_usage(),
                memory_mb,
                state,
                formatted_time,
                display_cmd
            );
        }
        
        if !continuous {
            break;
        }
        
        println!("\nRefreshing every {} seconds. Press Ctrl+C to exit.", interval);
        thread::sleep(Duration::from_secs(interval));
    }
}
