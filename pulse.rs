use std::{thread, time::Duration};
use std::io::{stdout, Write};
use std::process::Command;
use sysinfo::{System, ProcessStatus, Pid};
use users::get_user_by_uid;
use termion::event::Key;
use termion::input::TermRead;
// use termion::raw::IntoRawMode;
use termion::{cursor, clear};
use signal_hook::consts::signal::SIGINT;
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
mod restart;
mod priority;
use restart::{ProcessRestarter, RestartResult};
use std::fs;
use libc::{getpriority, PRIO_PROCESS};
use termion::raw::{IntoRawMode, RawTerminal};
use process_groups::ProcessGroupManager;





// Import the pause_resume module
mod pause_resume;
mod process_groups;

use pause_resume::{ProcessController, ProcessAction};

// Enum to track current sort mode
enum SortMode {
    Cpu,
    Memory,
    Pid,
}

// Enum to track current input mode
#[derive(PartialEq)]

enum InputMode {
    Normal,
    Search,
    Kill,
    Pause,  // New mode for pausing processes
    Restart,  // New mode for restarting processes
    Nice,
    Groups,
    Tree,
}

// fn prompt_password() -> String {
//     // Use rpassword which handles all the terminal mode complexity
//     match rpassword::prompt_password("Enter admin password: ") {
//         Ok(password) => password,
//         Err(_) => {
//             eprintln!("Failed to read password");
//             String::new() // Return empty string on error
//         }
//     }
// }

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
    let fg_color = "\x1B[38;5;201m"; // bright pink for FG
    let bg_color = "\x1B[38;5;39m";  // bright light blue for BG
    

    
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
    let mut group_manager = ProcessGroupManager::new();

    let mut show_tree = false;
    let mut tree_output = String::new();
    let mut tree_output_lines: Vec<String> = Vec::new();
    let mut tree_scroll = 0;
    let mut selected_index = 0;
    let visible_tree_height = 10; // or calculate from terminal height if needed


    // Inside the process listing loop, when printing process information
    // let nice = match priority::get_nice_value(pid.as_u32() as i32) {
    //     Ok(nice_value) => format!("{:+}", nice_value),  // Displaying nice value with sign
    //     Err(_) => String::from("N/A"),  // Handle errors (e.g., process not found)
    // };

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
        group_manager.force_update(&system); // or rely on internal interval logic

    
        
        // Move cursor to top left and clear screen
        write!(stdout, "{}", clear::All).unwrap(); // Clear entire screen
        write!(stdout, "{}", cursor::Goto(1, 1)).unwrap();
        
        // Get terminal size
        let (width, height) = termion::terminal_size().unwrap_or((80, 24));

        if input_mode == InputMode::Tree {
            write!(stdout, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap(); // FULL clear
            write!(stdout, "{}{}Process Tree (↑/↓, Enter, Esc){}\r\n\r\n", bold, header_color, reset).unwrap();

            for (i, line) in tree_output_lines.iter().enumerate().skip(tree_scroll).take(visible_tree_height) {
                if i == selected_index {
                    write!(stdout, "\x1B[7m{}\x1B[0m\r\n", line).unwrap(); // highlighted line
                } else {
                    write!(stdout, "{}\r\n", line).unwrap();
                }
            }
        }
        
        
        // Print the title with styling
        write!(stdout, "{}{}Pulse - Linux Process Monitor{}\r\n\r\n", title_color, bold, reset).unwrap();
        
        // Print the header with styled columns (fixed width to ensure alignment)
        write!(stdout, "{}{}",
            header_color, bold
        ).unwrap();
        
        // Column headers with padding to ensure alignment
        write!(stdout, "{:<6}  {:<15}  {:>6}  {:>6}  {:<6}  {:<6}  {:<10}  {:<30}\r\n", 
            "PID", "USER", "CPU%", "MEM%", "NICE", "FG/BG", "STATE", "COMMAND"
        ).unwrap();
    
        
        // Separator line
        write!(stdout, "{}{}\r\n", 
            separator_color,
            "─".repeat(width as usize)
        ).unwrap();
        
        write!(stdout, "{}", reset).unwrap();
        
        // Process list - collect as references to avoid ownership issues
        let mut processes: Vec<_> = system.processes().values().collect();
        
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
                    // First compare by memory usage (higher memory first)
                    let b_mem = b.memory();
                    let a_mem = a.memory();
                    
                    // If memory usage is significantly different, sort by that
                    if b_mem > a_mem + 1000000 || a_mem > b_mem + 1000000 {
                        b_mem.cmp(&a_mem)
                    } 
                    // If memory usage is similar, prioritize active processes
                    else {
                        // Consider process state, with running processes first
                        let a_running = a.status() == ProcessStatus::Run;
                        let b_running = b.status() == ProcessStatus::Run;
                        
                        match (a_running, b_running) {
                            // Both running or both not running - sort by memory
                            (true, true) | (false, false) => b_mem.cmp(&a_mem),
                            // a is running, b is not - a comes first
                            (true, false) => std::cmp::Ordering::Less,
                            // b is running, a is not - b comes first
                            (false, true) => std::cmp::Ordering::Greater,
                        }
                    }
                });
            },
            SortMode::Pid => {
                // First sort all processes by PID
                processes.sort_by(|a, b| a.pid().cmp(&b.pid()));
                
                // Then prioritize active processes (those with non-zero CPU usage)
                // This keeps the PID ordering but brings active processes to the top
                processes.sort_by(|a, b| {
                    let a_active = a.cpu_usage() > 0.1;
                    let b_active = b.cpu_usage() > 0.1;
                    
                    match (a_active, b_active) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.pid().cmp(&b.pid()) // Both active or both inactive, maintain PID order
                    }
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
        
        if input_mode != InputMode::Tree 
        {
        // Display processes
            for process in display_processes.iter().take(max_processes) {
                let pid: Pid = process.pid();
                let cpu = process.cpu_usage() as f64 / system.physical_core_count().unwrap_or(1) as f64;
                let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;
                // let pid: Pid = process.pid();
                let stat_path = format!("/proc/{}/stat", pid);
                let stat_content = fs::read_to_string(&stat_path).unwrap_or_default();
                let parts: Vec<&str> = stat_content.split_whitespace().collect();
                let pgrp  = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                let tpgid = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                let fg_bg = if pgrp == tpgid { "FG" } else { "BG" };
                let fg_bg_color = if fg_bg == "FG" { fg_color } else { bg_color };
                let pid_value = pid.as_u32() as u32; // Ensure correct type for getpriority
                //CHECK THIS PART
                // right after you've read stat_content:
                let parts: Vec<&str> = stat_content.split_whitespace().collect();
                let pid = process.pid(); // Ensure pid is fetched from the process
                let nice = match priority::get_nice_value(pid.as_u32() as i32) {  // Correct the pid here
                    Ok(nice_value) => format!("{}", nice_value),  // Displaying nice value with sign
                    Err(_) => String::from("0"),  // Handle errors (e.g., process not found)
                };
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
                let cpu_color = if cpu > 15.0 {
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
                    "{:<6}  {}{:<15}{}  {}{:>6.1}{}  {}{:>6.1}{}  {:<6}  {}{:<6}{}  {}{:<10}{}  {:<30}\r\n",
                    pid, 
                    user_color, username, reset,
                    cpu_color, cpu, reset,
                    mem_color, mem, reset,
                    nice,
                    fg_bg_color, fg_bg, reset,
                    state_color, state, reset,
                    command_display
                ).unwrap();
            }
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
            InputMode::Nice => {
                write!(stdout, "Set NICE (PID:): {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Groups =>{
                write!(stdout, "Enter Group ID to pause/resume: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            }
            InputMode::Normal => {
                write!(stdout, "Q:Quit | C:CPU | M:Mem | P:PID | S:Search | K:Kill | Z:Pause | R:Restart | N:Nice | G:Group Pause | T:Show Tree").unwrap();
            },
            InputMode::Tree => {
                write!(stdout, "Press Enter to select a process | Up/Down to navigate | Esc to exit").unwrap();
            },
            
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
                                    let pid = Pid::from(pid_val as usize);
                                    if process_controller.is_paused(&pid) {
                                        let _ = process_controller.control_process(pid, ProcessAction::Resume);
                                    } else {
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
                InputMode::Tree => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                        },
                        Key::Up => {
                            if selected_index > 0 {
                                selected_index -= 1;
                                if selected_index < tree_scroll {
                                    tree_scroll -= 1;
                                }
                            }
                        },
                        Key::Down => {
                            if selected_index + 1 < tree_output_lines.len() {
                                selected_index += 1;
                                if selected_index >= tree_scroll + visible_tree_height {
                                    tree_scroll += 1;
                                }
                            }
                        },
                        Key::Char('\n') => {
                            if let Some(line) = tree_output_lines.get(selected_index) {
                                if let Some(pid) = extract_pid_from_line(line) {
                                    pid_input = pid.to_string();
                                    input_mode = InputMode::Kill; // or Pause
                                }
                            }
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
                                    let pid = Pid::from(pid_val as usize);
                                    let result = process_restarter.restart_process(pid);
                                    status_message = match result {
                                        RestartResult::Success => format!("Process {} restart initiated", pid_val),
                                        RestartResult::KillFailed => format!("Failed to kill process {}", pid_val),
                                        RestartResult::NotFound => format!("Process {} not found", pid_val),
                                        RestartResult::RestartFailed => format!("Restart failed for process {}", pid_val),
                                        RestartResult::NotRunning => format!("Process {} is not running", pid_val),
                                        RestartResult::NoExecutable => format!("Could not determine executable for process {}", pid_val),
                                        RestartResult::Failed => format!("Failed to restart process {}", pid_val),
                                    };
                                    
                                    status_timer = 6;
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
                InputMode::Groups => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                if let Ok(pid_val) = pid_input.parse::<u32>() {
                                    let pid = Pid::from(pid_val as usize);
                                    let result = group_manager.toggle_process_group(&system, pid)
                                    ;
                                    status_message = match result {
                                        Ok(_) => format!("Process {} group toggled", pid_val),
                                        Err(e) => format!("Error: {}", e),
                                    };
                                    status_timer = 6;
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
                // When the user enters the Nice input mode
                InputMode::Nice => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if let Some((p_str, n_str)) = pid_input.split_once(':') {
                                if let (Ok(pid), Ok(nice)) = (p_str.parse::<i32>(), n_str.parse::<i32>()) {
                                    // Adjust the nice value without sudo
                                    match priority::set_priority(pid, nice) {
                                        Ok(msg)  => status_message = msg,
                                        Err(e)   => status_message = format!("Error: {}", e),
                                    }
                                    status_timer = 6;
                                } else {
                                    status_message = "Invalid PID:NICE format".into();
                                    status_timer = 6;
                                }
                            } else {
                                status_message = "Format must be PID:NICE".into();
                                status_timer = 6;
                            }
                
                            // Return to normal mode
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char(c) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                }
                
                InputMode::Normal => {
                    match key {
                        Key::Char('Q') => quit = true,
                        Key::Char('C') => sort_mode = SortMode::Cpu,
                        Key::Char('M') => sort_mode = SortMode::Memory,
                        Key::Char('P') => sort_mode = SortMode::Pid,
                        Key::Char('S') => {
                            input_mode = InputMode::Search;
                            search_query.clear();
                        },
                        Key::Char('K') => {
                            input_mode = InputMode::Kill;
                            pid_input.clear();
                        },
                        Key::Char('Z') => {
                            input_mode = InputMode::Pause;
                            pid_input.clear();
                        },
                        Key::Char('R') => {
                            input_mode = InputMode::Restart;
                            pid_input.clear();
                        },
                        Key::Char('N') => {
                            input_mode = InputMode::Nice;
                            pid_input.clear();
                        },
                        Key::Char('G') => {
                            input_mode = InputMode::Groups;
                            pid_input.clear();
                        },
                        Key::Char('T') => {
                            tree_output = group_manager.format_process_tree();
                            tree_output_lines = tree_output.lines().map(|s| s.to_string()).collect();
                            tree_scroll = 0;
                            selected_index = 0;
                            input_mode = InputMode::Tree;
                        }
                                              
                        _ => {}
                    }
                }
            }
        }
        
        // Use a shorter sleep duration for more responsive updates
        if input_mode != InputMode::Tree {
            thread::sleep(Duration::from_millis(500));
        } else {
            thread::sleep(Duration::from_millis(10)); // fast, responsive nav in tree mode
        }
         // Even shorter for better responsiveness
    }
    // Use the resume_all method to resume all paused processes before exiting
    process_controller.resume_all();
    
    // Clean up terminal
    write!(stdout, "{}{}", cursor::Show, clear::All).unwrap();
    stdout.flush().unwrap();

    fn extract_pid_from_line(line: &str) -> Option<u32> {
        // Looks for the last (number) in "name (1234)"
        if let Some(start) = line.rfind('(') {
            if let Some(end) = line[start..].find(')') {
                let pid_str = &line[start + 1..start + end];
                return pid_str.parse::<u32>().ok();
            }
        }
        None
    }
}
