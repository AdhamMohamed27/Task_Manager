use std::{thread, time::Duration};
use std::io::{stdout, Write};
use std::process::Command;
use std::fs;
use sysinfo::{System, ProcessStatus, Pid};
use users::get_user_by_uid;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{cursor, clear};
use signal_hook::consts::signal::SIGINT;
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
mod restart;
use restart::{ProcessRestarter, RestartResult};

// Import the pause_resume module
mod pause_resume;
use pause_resume::{ProcessController, ProcessAction};

mod reptyr;
use reptyr::attach_to_terminal;

// Enum to track current sort mode
enum SortMode {
    Cpu,
    Memory,
    Pid,
}

// Enum to track current input mode
enum InputMode {
    Normal,
    Search,
    Kill,
    Pause,  // New mode for pausing processes
    Restart,  // New mode for restarting processes
    Attach,
}

fn main() {
    // Set up terminal for raw mode
    let mut stdout = stdout().into_raw_mode().unwrap();
    let mut system = System::new_all();
    
    // Terminal colors and styles
    let reset = "\x1B[0m";
    let bold = "\x1B[1m";
    let header_color = "\x1B[38;5;39m"; // Bright blue for headers
    let title_color = "\x1B[38;5;82m";  // Green for title
    let separator_color = "\x1B[38;5;240m"; // Dark gray for separators
    let running_color = "\x1B[38;5;82m"; // Green for running processes
    let sleep_color = "\x1B[38;5;50m"; // Gray for sleeping processes
    let idle_color = "\x1B[38;5;201m"; // Pink for idle processes
    let stopped_color = "\x1B[38;5;124m"; // Red for stopped processes
    let zombie_color = "\x1B[38;5;27m"; // Blue for zombie processes
    let high_usage_color = "\x1B[38;5;196m"; // Red for high CPU/memory usage
    let medium_usage_color = "\x1B[38;5;220m"; // Yellow for medium usage
    let user_color = "\x1B[38;5;147m"; // Light purple for username
    let help_color = "\x1B[38;5;33m"; // Blue for help text
    let paused_color = "\x1B[38;5;208m"; // Orange for paused processes
    let restart_color = "\x1B[38;5;183m"; // Light purple for restart text
    let fg_color = "\x1B[38;5;201m";         // bright pink for FG
    let bg_color = "\x1B[38;5;39m";          // bright light blue for BG
    
    // Application state
    let mut sort_mode = SortMode::Cpu;
    let mut input_mode = InputMode::Normal;
    let mut search_query = String::new();
    let mut quit = false;
    let mut pid_input = String::new();
    let mut process_restarter = ProcessRestarter::new();
    let mut status_message = String::new();
    let mut status_timer = 0;
    
    // Process controller for tracking paused processes
    let mut process_controller = ProcessController::new();
    
    // Clear screen and hide cursor
    write!(stdout, "{}{}", clear::All, cursor::Hide).unwrap();
    stdout.flush().unwrap();
    
    // Setup Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    flag::register(SIGINT, running.clone()).unwrap();
    
    // Create input iterator ONCE for the main loop
    let mut input = termion::async_stdin().keys();
    
    while !quit && running.load(Ordering::Relaxed) {
        // Always refresh system data at the beginning of each loop
        system.refresh_all();
        
        // Move cursor to top left and clear screen
        write!(stdout, "{}", clear::All).unwrap(); // Clear entire screen
        write!(stdout, "{}", cursor::Goto(1, 1)).unwrap();
        
        // Get terminal size
        let (width, height) = termion::terminal_size().unwrap_or((80, 24));
        
        // Print the title with styling
        write!(stdout, "{}{}Pulse - Linux Process Monitor{}\r\n\r\n", title_color, bold, reset).unwrap();
        
        // Print the header with styled columns (fixed width to ensure alignment)
        write!(stdout, "{}{}",
            header_color, bold
        ).unwrap();
        
        // Column headers with padding to ensure alignment
        write!(stdout, "{:<6} {:<15} {:>6} {:>6} {:<3} {:<10} {:<30}\r\n", 
            "PID", "USER", "CPU%", "MEM%", "FG/BG", "STATE", "COMMAND"
        ).unwrap();
        
        // Separator line
        write!(stdout, "{}{}\r\n", 
            separator_color,
            "─".repeat(width as usize)
        ).unwrap();
        
        write!(stdout, "{}", reset).unwrap();
        
        // Process list - collect as references to avoid ownership issues
        let mut processes: Vec<_> = system.processes().values().collect();
        
        // FIXED SORTING LOGIC: Use explicit comparisons instead of negative keys
        match sort_mode {
            SortMode::Cpu => {
                processes.sort_by(|a, b| {
                    let b_cpu = b.cpu_usage();
                    let a_cpu = a.cpu_usage();
                    b_cpu.partial_cmp(&a_cpu).unwrap_or(std::cmp::Ordering::Equal)
                });
            },
            SortMode::Memory => {
                processes.sort_by(|a, b| {
                    let b_mem = b.memory();
                    let a_mem = a.memory();
                    b_mem.cmp(&a_mem)
                });
            },
            SortMode::Pid => {
                processes.sort_by(|a, b| {
                    a.pid().cmp(&b.pid())
                });
            },
        };
        
        // Filter processes if in search mode
        let display_processes = match input_mode {
            InputMode::Search if !search_query.is_empty() => {
                processes.iter()
                    .filter(|p| p.pid().to_string().contains(&search_query))
                    .collect::<Vec<_>>()
            },
            _ => processes.iter().collect()
        };
        
        // Calculate how many processes we can show
        let max_processes = height as usize - 8; // Account for header, footer, etc.
        
        // Display processes
        for process in display_processes.iter().take(max_processes) {
            let pid: Pid = process.pid();
            // Read /proc/[pid]/stat to determine FG/BG
            let stat_path = format!("/proc/{}/stat", pid);
            let stat_content = fs::read_to_string(&stat_path).unwrap_or_default();
            let parts: Vec<&str> = stat_content.split_whitespace().collect();
            let pgrp  = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            let tpgid = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            let fg_bg = if pgrp == tpgid { "FG" } else { "BG" };
            let fg_bg_color = if fg_bg == "FG" { fg_color } else { bg_color };

            let cpu = process.cpu_usage();
            let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;
            
            // Check if this process is paused by our app
            let is_paused = process_controller.is_paused(&pid);
            
            let mut state = match process.status() {
                ProcessStatus::Run => "Running",
                ProcessStatus::Sleep => "Sleep",
                ProcessStatus::Idle => "Idle",
                ProcessStatus::Stop => "Stopped",
                ProcessStatus::Zombie => "Zombie",
                _ => "Other",
            };
            
            // Override display if process is paused by our application
            if is_paused {
                state = "Paused";
            }
            
            // Color for state
            let state_color = if is_paused {
                paused_color
            } else {
                match process.status() {
                    ProcessStatus::Run => running_color,
                    ProcessStatus::Sleep => sleep_color,
                    ProcessStatus::Idle => idle_color,
                    ProcessStatus::Stop => stopped_color,
                    ProcessStatus::Zombie => zombie_color,
                    _ => reset,
                }
            };
            
            // Color for CPU based on usage
            let cpu_color = if cpu > 30.0 {
                high_usage_color
            } else if cpu > 10.0 {
                medium_usage_color
            } else {
                reset
            };
            
            // Color for memory based on usage
            let mem_color = if mem > 10.0 {
                high_usage_color
            } else if mem > 5.0 {
                medium_usage_color
            } else {
                reset
            };
            
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
            
            // Print process entry with fixed-width columns to ensure alignment
            write!(stdout, 
                "{:<6} {}{:<15}{} {}{:>6.1}{} {}{:>6.1}{} {}{:<3}{} {}{:<10}{} {:<30}\r\n",
                pid, 
                user_color, username, reset,
                cpu_color, cpu, reset,
                mem_color, mem, reset,
                fg_bg_color, fg_bg, reset,
                state_color, state, reset,
                command_display
            ).unwrap();
        }
        
        // Print system stats
        let mem_percent = (system.used_memory() as f64 / system.total_memory() as f64) * 100.0;
        let mem_gb = system.total_memory() as f64 / 1_073_741_824.0; // Convert to GB
        let mem_used_gb = system.used_memory() as f64 / 1_073_741_824.0;
        
        // Move to bottom of screen for stats
        let stats_line = height - 3;
        write!(stdout, "{}\r\n", cursor::Goto(1, stats_line)).unwrap();
        
        // Print memory and CPU info
        write!(stdout, "{}{}Memory: {:.1}GB / {:.1}GB ({:.1}%){}\r\n", 
            separator_color, bold, mem_used_gb, mem_gb, mem_percent, reset
        ).unwrap();
        
        let num_cores = system.physical_core_count().unwrap_or(1);
        let paused_count = process_controller.get_paused_processes().len();
        write!(stdout, "{}{}CPUs: {} cores, Processes: {}, Paused: {}{}", 
            separator_color, bold, num_cores, display_processes.len(), paused_count, reset
        ).unwrap();
        
        // Display status message if timer is active
        if status_timer > 0 {
            write!(stdout, " | {}{}{}", restart_color, status_message, reset).unwrap();
            status_timer -= 1;
        }
        
        write!(stdout, "\r\n").unwrap();
        
        // Help line at bottom
        write!(stdout, "{}{}", help_color, bold).unwrap();
        match input_mode {
            InputMode::Search => {
                write!(stdout, "Search PID: {} | Enter when Done | Esc to cancel", search_query).unwrap();
            },
            InputMode::Kill => {
                write!(stdout, "Kill PID: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Pause => {
                write!(stdout, "Enter PID to pause/resume: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Restart => {
                write!(stdout, "Enter PID to restart: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Attach => {
                write!(stdout,"Enter PID to change FG/BG: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },            
            InputMode::Normal => {
                // Updated key mappings in help text including restart
                write!(stdout, "q:Quit | c:CPU | m:Mem | p:PID | s:Search | k:Kill | z:Pause/Resume | r:Restart | g:Attach").unwrap();
            }
        }
        write!(stdout, "{}", reset).unwrap();
        
        // Make sure to flush stdout to display updates
        stdout.flush().unwrap();
        
        // Handle input (non-blocking)
        if let Some(Ok(key)) = input.next() {
            match input_mode {
                InputMode::Search => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            search_query.clear();
                        },
                        Key::Char('\n') => {
                            input_mode = InputMode::Normal;
                        },
                        Key::Char(c) if c.is_digit(10) => {
                            search_query.push(c);
                        },
                        Key::Backspace => {
                            search_query.pop();
                        },
                        _ => {}
                    }
                },
                InputMode::Kill => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                if let Ok(pid) = pid_input.parse::<i32>() {
                                    let _ = Command::new("kill")
                                        .arg(pid.to_string())
                                        .output();
                                }
                            }
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char(c) if c.is_digit(10) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
                InputMode::Pause => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                if let Ok(pid_val) = pid_input.parse::<u32>() {
                                    // Cast to usize when creating Pid (fixed from original code)
                                    let pid = Pid::from(pid_val as usize);
                                    
                                    // Check if process is already paused
                                    if process_controller.is_paused(&pid) {
                                        // Process is paused, so resume it
                                        let _ = process_controller.control_process(pid, ProcessAction::Resume);
                                    } else {
                                        // Process is not paused, so pause it
                                        let _ = process_controller.control_process(pid, ProcessAction::Pause);
                                    }
                                }
                            }
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char(c) if c.is_digit(10) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
                InputMode::Restart => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                if let Ok(pid_val) = pid_input.parse::<u32>() {
                                    // Create Pid from user input
                                    let pid = Pid::from(pid_val as usize);
                                    
                                    // Attempt to restart the process
                                    let result = process_restarter.restart_process(pid);
                                    
                                    // Set status message based on result
                                    status_message = match result {
                                        RestartResult::Success => format!("Process {} restart initiated", pid_val),
                                        RestartResult::KillFailed => format!("Failed to kill process {}", pid_val),
                                        RestartResult::NotFound => format!("Process {} not found", pid_val),
                                    };
                                    
                                    // Set timer to display message for a few cycles
                                    status_timer = 6; // Display for 3 seconds (6 * 500ms)
                                }
                            }
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char(c) if c.is_digit(10) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
                InputMode::Attach => {
                    match key {
                        Key::Esc => { input_mode = InputMode::Normal; pid_input.clear(); },
                        Key::Char('\n') => {
                            if let Ok(pid_val) = pid_input.parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);
                                match attach_to_terminal(pid) {
                                    Ok(_)   => status_message = format!("Attached {} to this TTY", pid_val),
                                    Err(e)  => status_message = e,
                                }
                                status_timer = 6;
                            }
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char(c) if c.is_digit(10) => pid_input.push(c),
                        Key::Backspace => { pid_input.pop(); },
                        _ => {}
                    }
                },
                InputMode::Normal => {
                    match key {
                        // Normal key mappings
                        Key::Char('q') => quit = true,
                        Key::Char('c') => sort_mode = SortMode::Cpu,
                        Key::Char('m') => sort_mode = SortMode::Memory,
                        Key::Char('p') => sort_mode = SortMode::Pid,
                        Key::Char('s') => {
                            input_mode = InputMode::Search;
                            search_query.clear();
                        },
                        Key::Char('k') => {
                            input_mode = InputMode::Kill;
                            pid_input.clear();
                        },
                        Key::Char('z') => {
                            input_mode = InputMode::Pause;
                            pid_input.clear();
                        },
                        Key::Char('r') => {
                            input_mode = InputMode::Restart;
                            pid_input.clear();
                        },
                        Key::Char('g') => { 
                            input_mode = InputMode::Attach; 
                            pid_input.clear(); 
                        },

                        // Keep F-key mappings as well for backward compatibility
                        Key::F(1) => quit = true,
                        Key::F(2) => sort_mode = SortMode::Cpu,
                        Key::F(3) => sort_mode = SortMode::Memory,
                        Key::F(4) => sort_mode = SortMode::Pid,
                        Key::F(5) => {
                            input_mode = InputMode::Search;
                            search_query.clear();
                        },
                        Key::F(9) => {
                            input_mode = InputMode::Kill;
                            pid_input.clear();
                        },
                        Key::F(10) => {
                            input_mode = InputMode::Pause;
                            pid_input.clear();
                        },
                        Key::F(11) => {
                            input_mode = InputMode::Restart;
                            pid_input.clear();
                        },
                        _ => {}
                    }
                },
            }
        }
        
        // Use a shorter sleep duration for more responsive updates
        thread::sleep(Duration::from_millis(500)); // Even shorter for better responsiveness
    }
    
    // Use the resume_all method to resume all paused processes before exiting
    process_controller.resume_all();
    
    // Clean up terminal
    write!(stdout, "{}{}", cursor::Show, clear::All).unwrap();
    stdout.flush().unwrap();
}
