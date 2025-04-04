use chrono::Local;
use std::{env, thread, time::Duration};
use sysinfo::{ProcessStatus, System};

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

    let mut system = System::new_all();

    loop {
        // Clear screen in continuous mode
        if continuous {
            print!("\x1B[2J\x1B[1;1H"); // ANSI escape code to clear screen
        }

        system.refresh_all();

        if show_header {
            println!(
                "{:<8} {:<10} {:<8} {:<10} {:<15} {:<20}",
                "PID", "CPU%", "MEM (MB)", "STATE", "TIME", "COMMAND"
            );
            println!("{:-<80}", "");
        }

        // Get all processes and sort by PID
        let mut processes: Vec<_> = system.processes().iter().collect();
        processes.sort_by_key(|&(pid, _)| pid);

        for (&pid, process) in processes {
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
                ProcessStatus::Unknown(_) => "Unknown",
                _ => "Other",
            };

            // Format memory usage in MB
            let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;

            // Just use the current time for display
            let current_time = Local::now().format("%H:%M:%S").to_string();

            // Get command with arguments
            let cmd = process.cmd().join(" ");
            let display_cmd = if cmd.is_empty() {
                process.name().to_string()
            } else {
                cmd
            };

            println!(
                "{:<8} {:<10.1} {:<10.1} {:<15} {:<15} {:.20}",
                pid,
                process.cpu_usage(),
                memory_mb,
                state,
                current_time,
                display_cmd
            );
        }

        if !continuous {
            break;
        }

        println!(
            "\nRefreshing every {} seconds. Press Ctrl+C to exit.",
            interval
        );
        thread::sleep(Duration::from_secs(interval));
    }
}
