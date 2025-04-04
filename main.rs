use std::{collections::HashMap, thread, time::Duration};
use sysinfo::{Pid, ProcessStatus, System};
use chrono::Local;

fn main() {
    // Initialize the system information gatherer
    let mut system = System::new_all();
    
    // Store previous process list to detect new and terminated processes
    let mut previous_processes: HashMap<Pid, String> = HashMap::new();
    
    println!("Real-time process monitor (Press Ctrl+C to exit)");
    println!("Watching for process changes...");

    loop {
        // Refresh the process list
        system.refresh_processes();
        
        // Get current process list
        let current_processes: HashMap<Pid, String> = system.processes()
            .iter()
            .map(|(pid, process)| (*pid, process.name().to_string()))
            .collect();
        
        // Check for new processes
        let mut new_processes = Vec::new();
        for (pid, name) in &current_processes {
            if !previous_processes.contains_key(pid) {
                new_processes.push((*pid, name.clone()));
            }
        }
        
        // Check for terminated processes
        let mut terminated_processes = Vec::new();
        for (pid, name) in &previous_processes {
            if !current_processes.contains_key(pid) {
                terminated_processes.push((*pid, name.clone()));
            }
        }
        
        // Display process changes if any occurred
        if !new_processes.is_empty() || !terminated_processes.is_empty() {
            let timestamp = Local::now().format("%H:%M:%S").to_string();
            
            // Clear screen for better readability
            print!("\x1B[2J\x1B[1;1H");
            
            println!("Process changes detected at {}", timestamp);
            println!("{:-<50}", "");
            
            // Show new processes with details
            if !new_processes.is_empty() {
                println!("\nNEW PROCESSES:");
                println!("{:<10} {:<10} {:<10} {:<15} {:<20}", 
                    "PID", "CPU%", "MEM (MB)", "STATE", "COMMAND");
                println!("{:-<65}", "");
                
                for (pid, _) in new_processes {
                    if let Some(process) = system.process(pid) {
                        // Format process state
                        let state = match process.status() {
                            ProcessStatus::Run => "Running",
                            ProcessStatus::Sleep => "Sleep",
                            ProcessStatus::Idle => "Idle",
                            ProcessStatus::Stop => "Stopped",
                            ProcessStatus::Zombie => "Zombie",
                            _ => "Other",
                        };
                        
                        // Format memory usage in MB
                        let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
                        
                        // Display process info with fixed width columns
                        println!(
                            "{:<10} {:<10.1} {:<10.1} {:<15} {:<20}",
                            pid,
                            process.cpu_usage(),
                            memory_mb,
                            state,
                            process.name()
                        );
                    }
                }
            }
            
            // Show terminated processes
            if !terminated_processes.is_empty() {
                println!("\nTERMINATED PROCESSES:");
                println!("{:<10} {:<20}", "PID", "NAME");
                println!("{:-<30}", "");
                
                for (pid, name) in terminated_processes {
                    println!("{:<10} {:<20}", pid, name);
                }
            }
            
            // Show current process count
            println!("\nTotal processes: {}", current_processes.len());
        }
        
        // Update previous process list for next iteration
        previous_processes = current_processes;
        
        // Brief sleep to reduce CPU usage
        thread::sleep(Duration::from_millis(500));
    }
}
