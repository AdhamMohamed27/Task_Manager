//Here we added two more functionalities changing a process from FG to BG and vice verse, and sending alerts when a process exceeds a certain CPU threshold. 

use std::{thread, time::Duration};
use std::io::{stdout, Write};
use std::process::Command;
use sysinfo::{System, Process, ProcessStatus, Pid};
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
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use std::collections::HashMap;

mod csv_export;
mod json_export;
mod help;
use help::get_help_text;

use csv_export::CsvExporter;
use json_export::JsonExporter;


// Import the pause_resume module
mod pause_resume;
mod process_groups;

//use std::time::{Instant};



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
    Export,
    JExport,
    FgBgSwitch,
    ThresholdConfig,
    Help,
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


// pub fn display_help_message() -> String {
//     let mut help = String::new();
//     use std::fmt::Write;

//     writeln!(help, "{}{}Pulse - Linux Process Monitor Help{}\r\n", "\x1B[1m", "\x1B[38;5;82m", "\x1B[0m").unwrap();

//     writeln!(help, "{}General Commands:{}\r", "\x1B[38;5;39m", "\x1B[0m").unwrap();
//     writeln!(help, "  Q       Quit the application\r").unwrap();
//     writeln!(help, "  C       Sort by CPU usage\r").unwrap();
//     writeln!(help, "  M       Sort by Memory usage\r").unwrap();
//     writeln!(help, "  P       Sort by PID\r").unwrap();
//     writeln!(help, "  S       Search by PID\r").unwrap();
//     writeln!(help, "  K       Kill a process\r").unwrap();
//     writeln!(help, "  Z       Pause/Resume a process\r").unwrap();
//     writeln!(help, "  R       Restart a process\r").unwrap();
//     writeln!(help, "  N       Set nice value (priority)\r").unwrap();
//     writeln!(help, "  G       Pause/Resume process group\r").unwrap();
//     writeln!(help, "  T       Show process tree view\r").unwrap();
//     writeln!(help, "  J       Export as JSON\r").unwrap();
//     writeln!(help, "  E       Export as CSV\r").unwrap();
//     writeln!(help, "  H       Show this help screen\r\n").unwrap();

//     writeln!(help, "{}Tree View Navigation:{}\r", "\x1B[38;5;39m", "\x1B[0m").unwrap();
//     writeln!(help, "  ↑ / ↓   Navigate process tree\r").unwrap();
//     writeln!(help, "  Enter   Select a process for action\r").unwrap();
//     writeln!(help, "  Esc     Exit tree view\r\n").unwrap();

//     writeln!(
//         help,
//         "{}Pulse{} is a real-time Linux process monitor that lets you sort, search, manage,\r\n\
//          and export process data. Use this interface to efficiently interact with running tasks.\r\n",
//         "\x1B[38;5;147m", "\x1B[0m"
//     )
//     .unwrap();

//     writeln!(help, "Press {}ESC{} to return.\r", "\x1B[1m", "\x1B[0m").unwrap();

//     help
// }


fn main() {
    let mut buffer = String::new();
    // Set up terminal for raw mode
    let mut stdout = stdout().into_raw_mode().unwrap();
    let mut system = System::new_all();
    // Set up terminal for raw mode
    // let mut stdout = stdout().into_raw_mode().unwrap();
    // let mut system = System::new_all();
    // let help_message = display_help_message();
    // write!(stdout, "{}", help_message).unwrap();
    // stdout.flush().unwrap();

    // // Wait for the user to press Enter
    // let mut input = termion::async_stdin().keys();
    // loop {
    //     if let Some(Ok(key)) = input.next() {
    //         match key {
    //             Key::Char('\n') => {
    //                 break; // User pressed Enter, exit the loop
    //             },
    //             _ => {}
    //         }
    //     }

    //     // You can add a small delay to reduce CPU load while waiting for input
    //     // thread::sleep(Duration::from_millis(100));
    // }

    
 
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

    let mut manual_fg_bg_map: HashMap<u32, String> = HashMap::new();

    let mut notifications: Vec<String> = vec![];
    let mut notification_timers: HashMap<u32, u32> = HashMap::new();
    let notification_duration = 4; // 5 seconds

    let mut cpu_threshold: f64 = 10.0;  // Default CPU Threshold
    let mut mem_threshold: f64 = 10.0;  // Default Memory Threshold


    // Inside the process listing loop, when printing process information
    // let nice = match priority::get_nice_value(pid.as_u32() as i32) {
    //     Ok(nice_value) => format!("{:+}", nice_value),  // Displaying nice value with sign
    //     Err(_) => String::from("N/A"),  // Handle errors (e.g., process not found)
    // };

    // Clear screen and hide cursor
    write!(buffer, "{}{}", clear::All, cursor::Hide).unwrap();
    stdout.flush().unwrap();
    
    // Setup Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    flag::register(SIGINT, running.clone()).unwrap();
    
    // Create input iterator ONCE for the main loop
    let mut input = termion::async_stdin().keys();

    let mut input = termion::async_stdin().keys();

    

    let help_screen = get_help_text();
    write!(buffer, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap();
    write!(buffer, "{}\r\n", help_screen).unwrap();
    stdout.write_all(buffer.as_bytes()).unwrap();
    stdout.flush().unwrap();
    buffer.clear();

    // Wait for ESC before entering Pulse
    loop {
        if let Some(Ok(key)) = input.next() {
            if key == Key::Char('\n') {
                break;
            }
        }
        // thread::sleep(Duration::from_millis(100));
    }
    
    while !quit && running.load(Ordering::Relaxed) {
        // Always refresh system data at the beginning of each loop

        // write!(buffer, "{}{}", clear::All, cursor::Hide).unwrap();
        // stdout.flush().unwrap();
        system.refresh_processes();
        group_manager.force_update(&system); // or rely on internal interval logic

    
        
        // // Move cursor to top left and clear screen
        // write!(buffer, "{}", clear::All).unwrap(); // Clear entire screen
        // stdout.flush().unwrap();
        // write!(buffer, "{}", cursor::Goto(1, 1)).unwrap();
        
        // Get terminal size
        let (width, height) = termion::terminal_size().unwrap_or((80, 24));

        if input_mode == InputMode::Tree {
            write!(buffer, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap(); // FULL clear
            write!(buffer, "{}{}Process Tree (↑/↓, Enter, Esc){}\r\n\r\n", bold, header_color, reset).unwrap();

            for (i, line) in tree_output_lines.iter().enumerate().skip(tree_scroll).take(visible_tree_height) {
                if i == selected_index {
                    write!(buffer, "\x1B[7m{}\x1B[0m\r\n", line).unwrap(); // highlighted line
                } else {
                    write!(buffer, "{}\r\n", line).unwrap();
                }
            }
        }
        
        
        // Print the title with styling
        write!(buffer, "{}{}", cursor::Goto(1, 1), clear::AfterCursor).unwrap();  // Clear everything below
        write!(buffer, "{}{}Pulse - Linux Process Monitor{}\r\n\r\n", title_color, bold, reset).unwrap();
        
        // Print the header with styled columns (fixed width to ensure alignment)
        write!(buffer, "{}{}",
            header_color, bold
        ).unwrap();

        // Column headers with padding to ensure alignment
        write!(buffer, "{:<6}  {:<15}  {:>6}  {:>6}  {:<6}  {:<6}  {:<10}  {:<30}\r\n", 
            "PID", "USER", "CPU%", "MEM%", "NICE", "FG/BG", "STATE", "COMMAND"
        ).unwrap();
    
        
        // Separator line
        write!(buffer, "{}{}\r\n", 
            separator_color,
            "─".repeat(width as usize)
        ).unwrap();
        
        write!(buffer, "{}", reset).unwrap();
        
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
        let display_processes: Vec<&Process> = match input_mode {
            InputMode::Search if !search_query.is_empty() => {
                processes.iter()
                    .filter(|p| p.pid().to_string().contains(&search_query))
                    .copied() // <- this turns &Process into Process
                    .collect()
            },
            InputMode::FgBgSwitch => {
                processes.iter()
                    .filter(|p| p.pid().to_string().contains(&search_query))
                    .copied() // <- this turns &Process into Process
                    .collect()
            },
            _ => processes.iter().copied().collect(), // <- same here
        };

        // // Initialize the cached processes and refresh control
        // let mut cached_processes: Vec<&Process> = vec![];
        // let mut last_refresh = Instant::now();
        // let refresh_interval = Duration::from_millis(500); // 500 milliseconds


        // // Refresh cached processes only every 500ms
        // if last_refresh.elapsed() > refresh_interval {
        //     cached_processes = processes.iter().copied().collect();
        //     last_refresh = Instant::now();
        // }
        // // Initialize the display processes list
        // let mut display_processes: Vec<&Process> = processes.iter().copied().collect();

        // // Handle filtering for search and FG/BG mode
        // if input_mode == InputMode::Search && !search_query.is_empty() {
        //     display_processes.retain(|p| p.pid().to_string().contains(&search_query));
        // } else if input_mode == InputMode::FgBgSwitch {
        //     if !search_query.is_empty() {
        //         display_processes.retain(|p| p.pid().to_string().contains(&search_query));
        //     }
        // }
        
        
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

                // Monitor CPU and Memory thresholds for notifications
                // Corrected Notification Trigger with PID as u32
                let pid_u32 = pid.as_u32(); // Convert PID to u32

                if cpu > cpu_threshold {
                    let message = format!("ALERT: Process '{}' (PID: {}) exceeded CPU threshold ({:.2}%)", process.name(), pid, cpu);
                    if !notification_timers.contains_key(&pid_u32) {
                        notifications.push(message);
                        notification_timers.insert(pid_u32, notification_duration);
                    }
                } else if mem > mem_threshold {
                    let message = format!("ALERT: Process '{}' (PID: {}) exceeded Memory threshold ({:.2}%)", process.name(), pid, mem);
                    if !notification_timers.contains_key(&pid_u32) {
                        notifications.push(message);
                        notification_timers.insert(pid_u32, notification_duration);
                    }
                }

                // Determine FG/BG state (Manual if set, else system detected)
                // Determine FG/BG state (Manual if set, else system detected)
                let stat_path = format!("/proc/{}/stat", pid);
                let stat_content = fs::read_to_string(&stat_path).unwrap_or_default();
                let parts: Vec<&str> = stat_content.split_whitespace().collect();
                let pgrp  = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                let tpgid = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);

                let fg_bg = if let Some(manual_status) = manual_fg_bg_map.get(&pid.as_u32()) {
                    manual_status.clone()
                } else {
                    // Default behavior (detected from system)
                    if pgrp == tpgid { "FG".to_string() } else { "BG".to_string() }
                };

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
                write!(buffer, 
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

                // if input_mode == InputMode::Normal {
                //     if pid_input.trim().to_lowercase() == "help" {
                //         // Display help message with functionality
                //         display_help_message(&mut stdout);
                //         pid_input.clear(); // Clear the help command after displaying
                //     }   
                // }    
            }

            stdout.flush().unwrap();
        }
        
        // Print system stats
        let mem_percent = (system.used_memory() as f64 / system.total_memory() as f64) * 100.0;
        let mem_gb = system.total_memory() as f64 / 1_073_741_824.0; // Convert to GB
        let mem_used_gb = system.used_memory() as f64 / 1_073_741_824.0;
        
        // Move to bottom of screen for stats
        let stats_line = height - 3;
        write!(buffer, "{}\r\n", cursor::Goto(1, stats_line)).unwrap();

        // Display stacked notifications
        notifications.retain(|message| {
            // Extract the PID from the message
            if let Some(pid) = message.split_whitespace().nth(4).and_then(|s| s.trim_start_matches('(').trim_end_matches(')').parse::<u32>().ok()) {
                if let Some(timer) = notification_timers.get_mut(&pid) {
                    *timer -= 1;
                    if *timer == 0 {
                        notification_timers.remove(&pid);
                        return false; // Remove this notification
                    }
                }
            }
            true // Retain this notification
        });

        // Render notifications at the bottom of the screen
        for message in &notifications {
            write!(buffer, "{}{}{}\r\n", "\x1B[38;5;196m", message, reset).unwrap();
        }

        // Display CPU and Memory Thresholds in Header
        write!(buffer, "Current Thresholds - CPU: {:.2}% | Memory: {:.2}%\r\n", cpu_threshold, mem_threshold).unwrap(); 
        
        // Print memory and CPU info
        write!(buffer, "{}{}Memory: {:.1}GB / {:.1}GB ({:.1}%){}\r\n", 
            separator_color, bold, mem_used_gb, mem_gb, mem_percent, reset
        ).unwrap();
        
        let num_cores = system.physical_core_count().unwrap_or(1);
        let paused_count = process_controller.get_paused_processes().len();
        write!(buffer, "{}{}CPUs: {} cores, Processes: {}, Paused: {}{}", 
            separator_color, bold, num_cores, display_processes.len(), paused_count, reset
        ).unwrap();


        // Display status message if timer is active
        if status_timer > 0 {
            write!(buffer, " | {}{}{}", restart_color, status_message, reset).unwrap();
            status_timer -= 1;
        }
        
        write!(buffer, "\r\n").unwrap();

        
        // Help line at bottom
        write!(buffer, "{}{}", help_color, bold).unwrap();
        match input_mode {
            InputMode::Search => {
                write!(buffer, "Search PID: {} | Enter when Done | Esc to cancel", search_query).unwrap();
            },
            InputMode::Kill => {
                write!(buffer, "Kill PID: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Pause => {
                write!(buffer, "Enter PID to pause/resume: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Restart => {
                write!(buffer, "Enter PID to restart: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Nice => {
                write!(buffer, "Set NICE (PID:): {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::Groups =>{
                write!(buffer, "Enter Group ID to pause/resume: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::FgBgSwitch => { 
                write!(buffer, "FG/BG Mode | Enter PID: {} | Enter to switch | Esc to cancel", search_query).unwrap();
            },
            InputMode::ThresholdConfig => {
                write!(buffer, "Set Threshold | Type 'CPU' or 'MEM' | Esc to cancel: {}", pid_input).unwrap();
                        },
            InputMode::Normal => {
                write!(buffer, "Q:Quit | C:CPU | M:Mem | P:PID | S:Search | K:Kill | Z:Pause | R:Restart | N:Nice | G:Group Pause | F:Fg/Bg Switch | W:Set Threshold | T:Show Tree | J: Export as Json file | E: Export as CSV file | H: Help").unwrap();
            },
            InputMode::Tree => {
                write!(buffer, "Press Enter to select a process | Up/Down to navigate | Esc to exit").unwrap();
            },
            InputMode::Export => {
                write!(buffer, "Export to CSV: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },
            InputMode::JExport => {
                write!(buffer, "Export to JSON: {} | Enter to confirm | Esc to cancel", pid_input).unwrap();
            },   
            InputMode::Help => {
                write!(buffer , "Press Enter to view help | Esc to cancel").unwrap();
            }, 
        }
        write!(buffer, "{}", reset).unwrap();
        
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
                InputMode::Help => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            let help_text = get_help_text();
                            write!(buffer, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap();
                            write!(buffer, "{}{}{}\r\n\r\n", help_color, bold, help_text).unwrap();
                            input_mode = InputMode::Normal;
                        },
                        _ => {}
                    }
                },
                InputMode::Export => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                let file_path = pid_input.clone();
                                let flattened_processes: Vec<&Process> = display_processes.iter().copied().collect();
                                let result = CsvExporter::export_processes(&flattened_processes, &system, &file_path);
                                status_message = match result {
                                    Ok(msg) => msg,
                                    Err(e) => format!("Error: {}", e),
                                };
                                status_timer = 6;
                            }
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        }
                        ,
                        Key::Char(c) if c.is_digit(10) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
                InputMode::JExport => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            if !pid_input.is_empty() {
                                let file_path = pid_input.clone();
                                let flattened_processes: Vec<&Process> = display_processes.iter().copied().collect();
                                let result = JsonExporter::export(&flattened_processes, &system, &file_path);
                                status_message = match result {
                                    Ok(msg) => msg,
                                    Err(e) => format!("Error: {}", e),
                                };
                                status_timer = 6;
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

                InputMode::FgBgSwitch => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            search_query.clear();
                        },
                        Key::Char('\n') => {
                            if let Ok(pid_val) = search_query.trim().parse::<u32>() {
                                println!("Switch FG/BG for PID {}? (y/n)", pid_val);
                                stdout.flush().unwrap(); // Ensure the message is displayed immediately
                
                                // Wait for user confirmation without clearing the screen
                                loop {
                                    if let Some(Ok(confirm)) = input.next() {
                                        if confirm == Key::Char('y') {
                                            match toggle_fg_bg(pid_val) {
                                                Ok(_) => {
                                                    if let Some(current) = manual_fg_bg_map.get(&pid_val) {
                                                        if current == "FG" {
                                                            manual_fg_bg_map.insert(pid_val, "BG".to_string());
                                                        } else {
                                                            manual_fg_bg_map.insert(pid_val, "FG".to_string());
                                                        }
                                                    } else {
                                                        manual_fg_bg_map.insert(pid_val, "BG".to_string());
                                                    }
                                                    status_message = format!("Attempted to switch FG/BG for PID {}", pid_val);
                                                },
                                                Err(e) => {
                                                    manual_fg_bg_map.insert(pid_val, "BG".to_string());
                                                    //status_message = format!("Failed to switch FG/BG for PID {}: {}", pid_val, e);
                                                },
                                            }
                                        } else if confirm == Key::Char('n') {
                                            status_message = "Cancelled switching FG/BG.".to_string();
                                        }
                                        break;
                                    }
                                }
                
                                // After confirmation, we must clear and refresh the screen
                                write!(stdout, "{}", clear::All).unwrap();
                                write!(stdout, "{}{}", cursor::Goto(1, 1), status_message).unwrap();
                                stdout.flush().unwrap();
                            } else {
                                status_message = "Invalid PID format.".to_string();
                            }
                
                            input_mode = InputMode::Normal;
                            search_query.clear();
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
                },
                InputMode::ThresholdConfig => {
                    match key {
                        Key::Esc => {
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Char('\n') => {
                            let choice = pid_input.trim().to_uppercase();
                            
                            if choice == "CPU" {
                                pid_input.clear();
                                write!(buffer, "Set CPU Threshold (Current: {:.2}%) | Enter to confirm | Esc to cancel: ", cpu_threshold).unwrap();
                                stdout.write_all(buffer.as_bytes()).unwrap();
                                stdout.flush().unwrap();
                                status_message = "Please enter the CPU threshold value.".to_string();
                            } else if choice == "MEM" {
                                pid_input.clear();
                                write!(buffer, "Set Memory Threshold (Current: {:.2}%) | Enter to confirm | Esc to cancel: ", mem_threshold).unwrap();
                                stdout.write_all(buffer.as_bytes()).unwrap();
                                stdout.flush().unwrap();
                                status_message = "Please enter the Memory threshold value.".to_string();
                            } else if let Ok(value) = choice.parse::<f64>() {
                                if status_message.contains("CPU") {
                                    cpu_threshold = value;
                                    status_message = format!("CPU Threshold set to {:.2}%", cpu_threshold);
                                } else if status_message.contains("Memory") {
                                    mem_threshold = value;
                                    status_message = format!("Memory Threshold set to {:.2}%", mem_threshold);
                                } else {
                                    status_message = "Invalid option. Type 'CPU' or 'MEM' first.".to_string();
                                }
                                pid_input.clear();
                                input_mode = InputMode::Normal;
                                stdout.write_all(buffer.as_bytes()).unwrap();
                                stdout.flush().unwrap();
                            } else {
                                status_message = "Invalid input. Type 'CPU', 'MEM', or a numeric value.".to_string();
                                stdout.write_all(buffer.as_bytes()).unwrap();
                                stdout.flush().unwrap();
                            }
                        },

                        Key::Char(c) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
                
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
                        },
                        Key::Char('F') => {
                            input_mode = InputMode::FgBgSwitch;
                            search_query.clear();
                        },     
                        Key::Char('W') => {
                            input_mode = InputMode::ThresholdConfig;
                            pid_input.clear();
                        },                   
                        Key::Char('E') => {
                            let filename = CsvExporter::get_default_filename();
                            let filepath = format!("/home/adham-mohamed/Desktop/{}", filename);
                        
                            // Create flattened list only when exporting
                            let flattened_processes: Vec<&Process> = system.processes().values().collect();
                        
                            match CsvExporter::export_processes(&flattened_processes, &system, &filepath) {
                                Ok(msg) => {
                                    status_message = msg;
                                    status_timer = 6;
                                },
                                Err(e) => {
                                    status_message = format!("Export failed: {}", e);
                                    status_timer = 6;
                                }
                            }
                        },
                        Key::Char('H') => {
                            let help_screen = get_help_text();
                            write!(buffer, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap();
                            write!(buffer, "{}", help_screen).unwrap();
                            stdout.write_all(buffer.as_bytes()).unwrap();
                            stdout.flush().unwrap();
                            buffer.clear();
                        
                            // Wait for key press to continue
                            loop {
                                if let Some(Ok(key)) = input.next() {
                                    if key == Key::Char('\n') || key == Key::Char('h') {
                                        break;
                                    }
                                }
                                thread::sleep(Duration::from_millis(100));
                            }
                        },                          
                        
                        Key::Char('J') => {
                            let filename = JsonExporter::get_default_filename();
                            let filepath = format!("/home/adham-mohamed/Desktop/{}", filename);
                        
                            // Only collect processes here
                            let flattened_processes: Vec<&Process> = system.processes().values().collect();
                        
                            match JsonExporter::export(&flattened_processes, &system, &filepath) {
                                Ok(msg) => {
                                    status_message = msg;
                                    status_timer = 6;
                                },
                                Err(e) => {
                                    status_message = format!("Export failed: {}", e);
                                    status_timer = 6;
                                }
                            }
                        },                                               
                                              
                        _ => {}
                    }
                }
            }
        }
        
        // Use a shorter sleep duration for more responsive updates
        if input_mode != InputMode::Tree {
            thread::sleep(Duration::from_millis(1000));
        } else {
            thread::sleep(Duration::from_millis(10)); // fast, responsive nav in tree mode
        }
         // Even shorter for better responsiveness

        stdout.write_all(buffer.as_bytes()).unwrap();
        stdout.flush().unwrap();
        buffer.clear();
 
         // Responsive sleep
        if input_mode != InputMode::Tree {
            thread::sleep(Duration::from_millis(500));
        } else {
            thread::sleep(Duration::from_millis(10));
        } 
    }
    // Use the resume_all method to resume all paused processes before exiting
    process_controller.resume_all();
    
    // Clean up terminal
    write!(buffer, "{}{}", cursor::Show, clear::All).unwrap();
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

    // Function to toggle FG/BG for a process by PID
    fn toggle_fg_bg(pid: u32) -> Result<(), String> {
        let stat_path = format!("/proc/{}/stat", pid);
        let stat_content = std::fs::read_to_string(&stat_path).map_err(|_| "Failed to read process stat file")?;
        let parts: Vec<&str> = stat_content.split_whitespace().collect();

        let pgrp = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let tpgid = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);

        if pgrp == tpgid {
            Command::new("kill")
                .arg("-SIGTTOU") // Send process to background
                .arg(pid.to_string())
                .output()
                .map_err(|_| "Failed to switch process to background")?;
        } else {
            Command::new("kill")
                .arg("-SIGTTIN") // Bring process to foreground
                .arg(pid.to_string())
                .output()
                .map_err(|_| "Failed to switch process to foreground")?;
        }

        Ok(())
    }

    
    
}
