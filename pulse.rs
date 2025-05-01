use std::{thread, time::Duration};
use std::io::{stdout, Write};
use std::process::Command;
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
use std::fs;
use libc::{getpriority, PRIO_PROCESS};
use rpassword;

mod restart;
mod priority;
mod pause_resume;
mod process_groups;

use restart::{ProcessRestarter, RestartResult};
use pause_resume::{ProcessController, ProcessAction};
use process_groups::ProcessGroupManager;

// Enum to track current sort mode
enum SortMode {
    Cpu,
    Memory,
    Pid,
    Group, // New sort mode for process groups
}

// Enum to track current input mode
enum InputMode {
    Normal,
    Search,
    Kill,
    Pause,
    Restart,
    Priority,
    GroupView, // New mode for viewing process groups
    GroupAction, // New mode for actions on a process group
}

// Enum to represent view states
enum ViewState {
    ProcessList,
    GroupView,
}

fn prompt_password() -> String {
    // Use rpassword which handles all the terminal mode complexity
    match rpassword::prompt_password("Enter admin password: ") {
        Ok(password) => password,
        Err(_) => {
            eprintln!("Failed to read password");
            String::new() // Return empty string on error
        }
    }
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
    let fg_color = "\x1B[38;5;201m"; // bright pink for FG
    let bg_color = "\x1B[38;5;39m";  // bright light blue for BG
    let group_color = "\x1B[38;5;118m"; // Bright green for process groups
    
    // Application state
    let mut sort_mode = SortMode::Cpu;
    let mut input_mode = InputMode::Normal;
    let mut view_state = ViewState::ProcessList;
    let mut search_query = String::new();
    let mut quit = false;
    let mut pid_input = String::new();
    let mut process_restarter = ProcessRestarter::new();
    let mut status_message = String::new();
    let mut status_timer = 0;
    let mut force_refresh = false; // Flag to force immediate refresh
    let mut selected_group_pid: Option<Pid> = None;

    // Initialize process controller and process group manager
    let mut process_controller = ProcessController::new();
    let mut process_group_manager = ProcessGroupManager::new();
    
    // Clear screen and hide cursor
    write!(stdout, "{}", cursor::Goto(1, 1)).unwrap(); // Only move cursor to top-left
    write!(stdout, "{}{:width$}", cursor::Goto(1, 5), "", width = 80).unwrap(); // clear line

    stdout.flush().unwrap();
    
    // Setup Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    flag::register(SIGINT, running.clone()).unwrap();
    
    // Create input iterator ONCE for the main loop
    let mut input = termion::async_stdin().keys();
    
    // Set a slower refresh rate - 500ms instead of 100ms
    let refresh_duration = Duration::from_millis(1000);
    
    while !quit && running.load(Ordering::Relaxed) {
        // Always refresh system data at the beginning of each loop
        system.refresh_processes();        
        // Move cursor to top left and clear screen
        write!(stdout, "{}", clear::All).unwrap(); // Clear entire screen
        write!(stdout, "{}", cursor::Goto(1, 1)).unwrap();
        
        // Get terminal size
        let (width, height) = termion::terminal_size().unwrap_or((80, 24));
        
        // Print the title with styling
        write!(stdout, "{}{}Pulse - Linux Process Monitor{}\r\n\r\n", title_color, bold, reset).unwrap();
        
        // Build process groups
        let process_groups = process_group_manager.build_process_groups(&system);
        
        match view_state {
            ViewState::ProcessList => {
                // Normal process list view
                
                // Print the header with styled columns (fixed width to ensure alignment)
                write!(stdout, "{}{}",
                    header_color, bold
                ).unwrap();
                
                // Column headers with padding to ensure alignment
                write!(stdout, "{:<6}  {:<15}  {:>6}  {:>6}  {:<6}  {:<6}  {:<10}  {:<30}\r\n", 
                    "PID", "USER", "CPU%", "MEM%", "PRIO", "FG/BG", "STATE", "COMMAND"
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
                                _ => std::cmp::Ordering::Equal // Both active or both inactive, maintain existing order
                            }
                        });
                    },
                    SortMode::Group => {
                        // Sort by parent PID (PPID)
                        processes.sort_by(|a, b| {
                            let a_ppid = a.parent().unwrap_or(Pid::from(0));
                            let b_ppid = b.parent().unwrap_or(Pid::from(0));
                            a_ppid.cmp(&b_ppid)
                        });
                    },
                };
                
                // Filter processes if in search mode
                let display_processes = match input_mode {
                    InputMode::Search if !search_query.is_empty() => {
                        processes.iter()
                            .filter(|p| p.name().contains(&search_query) || 
                                   p.pid().to_string().contains(&search_query))
                            .collect::<Vec<_>>()
                    },
                    _ => processes.iter().collect()
                };
                
                // Calculate how many processes we can show
                let max_processes = height as usize - 8; // Account for header, footer, etc.
                
                // Display processes
                for process in display_processes.iter().take(max_processes) {
                    let pid: Pid = process.pid();
                    let cpu = process.cpu_usage();
                    let mem = (process.memory() as f64 / system.total_memory() as f64) * 100.0;
                    let stat_path = format!("/proc/{}/stat", pid);
                    
                    // Safely read stat content, using default values if it fails
                    let (pgrp, tpgid) = match fs::read_to_string(&stat_path) {
                        Ok(content) => {
                            let parts: Vec<&str> = content.split_whitespace().collect();
                            let pg = parts.get(4).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                            let tpg = parts.get(7).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
                            (pg, tpg)
                        },
                        Err(_) => (0, 0) // Default values if file can't be read
                    };
                    
                    let fg_bg = if pgrp == tpgid { "FG" } else { "BG" };
                    let fg_bg_color = if fg_bg == "FG" { fg_color } else { bg_color };
                    
                    // Get process priority safely
                    let pid_value = pid.as_u32() as u32;
                    let priority = unsafe {
                        let prio = getpriority(PRIO_PROCESS, pid_value);
                        if prio == -1 && nix::errno::errno() != 0 {
                            "N/A".to_string()
                        } else {
                            prio.to_string()
                        }
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
                        "{:<6}  {}{:<15}{}  {}{:>6.1}{}  {}{:>6.1}{}  {:<6}  {}{:<6}{}  {}{:<10}{}  {:<30}\r\n",
                        pid, 
                        user_color, username, reset,
                        cpu_color, cpu, reset,
                        mem_color, mem, reset,
                        priority,
                        fg_bg_color, fg_bg, reset,
                        state_color, state, reset,
                        command_display
                    ).unwrap();

                    
                }
                
                // Get a fresh copy of paused processes
                let paused_processes = process_controller.get_paused_processes();
                
                // Display paused processes table (only if any processes are paused)
                if !paused_processes.is_empty() {
                    // Move cursor down a bit to separate from process list
                    write!(stdout, "\r\n").unwrap();
                    
                    // Header for paused processes table
                    write!(stdout, "{}{}Paused Processes:{}\r\n", paused_color, bold, reset).unwrap();
                    
                    // Column headers with padding to ensure alignment
                    write!(stdout, "{}{}{:<6}  {:<30}{}\r\n", 
                        header_color, bold, "PID", "COMMAND", reset
                    ).unwrap();
                    
                    // Separator line for paused table
                    write!(stdout, "{}{}\r\n", 
                        separator_color,
                        "─".repeat(38)
                    ).unwrap();
                    
                    // Collect PIDs to remove (terminated processes)
                    let mut pids_to_remove = Vec::new();
                    
                    // Create a copy of the paused processes for safe iteration
                    let paused_pids: Vec<Pid> = paused_processes.clone();
                    
                    // Display each paused process
                    for &pid in &paused_pids {
                        // Find process in system to get name
                        if let Some(process) = system.process(pid) {
                            let command = process.name();
                            let command_display = if command.len() > 29 {
                                format!("{}...", &command[..26])
                            } else {
                                command.to_string()
                            };
                            
                            // Print paused process entry
                            write!(stdout, 
                                "{:<6}  {}{:<30}{}\r\n",
                                pid, 
                                paused_color, command_display, reset
                            ).unwrap();
                        } else {
                            // Process might have terminated but still in our paused list
                            write!(stdout, 
                                "{:<6}  {}<process terminated>{}\r\n",
                                pid, 
                                paused_color, reset
                            ).unwrap();
                            
                            // Mark this PID for removal
                            pids_to_remove.push(pid);
                        }
                    }
                    
                    // Remove terminated processes from our paused list
                    for pid in pids_to_remove {
                        process_controller.remove_terminated_process(&pid);
                        process_group_manager.remove_terminated_process(&pid);
                    }
                }
            },
            ViewState::GroupView => {
                // Process groups view
                
                // Print the header with styled columns
                write!(stdout, "{}{}PROCESS GROUPS{}\r\n\r\n", group_color, bold, reset).unwrap();
                
                // Column headers with padding to ensure alignment
                write!(stdout, "{}{}",
                    header_color, bold
                ).unwrap();
                
                write!(stdout, "{:<6}  {:<30}  {:>6}  {:<10}  {:<10}\r\n", 
                    "PPID", "PARENT COMMAND", "CHILD#", "CPU%", "STATUS"
                ).unwrap();
            
                // Separator line
                write!(stdout, "{}{}\r\n", 
                    separator_color,
                    "─".repeat(width as usize)
                ).unwrap();
                
                write!(stdout, "{}", reset).unwrap();
                
                // Calculate how many groups we can show
                let max_groups = height as usize - 8; // Account for header, footer, etc.
                
                // Build and display process groups
                let process_groups = process_group_manager.build_process_groups(&system);
                
                // Filter groups to only show those with at least one child
                let display_groups: Vec<_> = process_groups.iter()
                    .filter(|group| !group.children.is_empty())
                    .take(max_groups)
                    .collect();
                
                for group in display_groups {
                    // Get all processes in the group
                    let pids = process_group_manager.get_group_pids(&system, group.parent_pid);
                    
                    // Calculate total CPU for the group
                    let mut total_cpu = 0.0;
                    for &pid in &pids {
                        if let Some(process) = system.process(pid) {
                            total_cpu += process.cpu_usage();
                        }
                    }
                    
                    // Check if group is paused
                    let is_group_paused = process_group_manager.is_group_paused(&system, group.parent_pid);
                    let status_str = if is_group_paused { "PAUSED" } else { "ACTIVE" };
                    let status_color = if is_group_paused { paused_color } else { running_color };
                    
                    // Color for CPU based on usage
                    let cpu_color = if total_cpu > 30.0 {
                        high_usage_color
                    } else if total_cpu > 10.0 {
                        medium_usage_color
                    } else {
                        reset
                    };
                    
                    write!(stdout, 
                        "{:<6}  {:<30}  {:>6}  {}{:>10.1}{}  {}{:<10}{}\r\n",
                        group.parent_pid,
                        group.parent_name,
                        group.children.len(),
                        cpu_color, total_cpu, reset,
                        status_color, status_str, reset
                    ).unwrap();
                }
                
                // If a group is selected, show its child processes
                if let Some(parent_pid) = selected_group_pid {
                    // Find the group
                    if let Some(group) = process_groups.iter().find(|g| g.parent_pid == parent_pid) {
                        write!(stdout, "\r\n{}{}Child processes of {} ({}):{}\r\n",
                            group_color, bold, parent_pid, group.parent_name, reset
                        ).unwrap();
                        
                        // Column headers for child processes
                        write!(stdout, "{}{}",
                            header_color, bold
                        ).unwrap();
                        
                        write!(stdout, "{:<6}  {:<30}  {:>6}  {:<10}\r\n", 
                            "PID", "COMMAND", "CPU%", "STATUS"
                        ).unwrap();
                        
                        // Separator line
                        write!(stdout, "{}{}\r\n", 
                            separator_color,
                            "─".repeat(50)
                        ).unwrap();
                        
                        write!(stdout, "{}", reset).unwrap();
                        
                        // Get all children (including nested)
                        let child_pids = process_group_manager.get_group_pids(&system, parent_pid);
                        
                        // Skip the parent (first element)
                        for &pid in child_pids.iter().skip(1).take(10) { // Limit to 10 children to avoid clutter
                            if let Some(process) = system.process(pid) {
                                let command = process.name();
                                let command_display = if command.len() > 29 {
                                    format!("{}...", &command[..26])
                                } else {
                                    command.to_string()
                                };
                                
                                let cpu = process.cpu_usage();
                                let is_paused = process_controller.is_paused(&pid);
                                
                                let status = if is_paused {
                                    "PAUSED"
                                } else {
                                    match process.status() {
                                        ProcessStatus::Run => "Running",
                                        ProcessStatus::Sleep => "Sleep",
                                        ProcessStatus::Idle => "Idle",
                                        ProcessStatus::Stop => "Stopped",
                                        ProcessStatus::Zombie => "Zombie",
                                        _ => "Other",
                                    }
                                };
                                
                                let status_color = if is_paused {
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
                                
                                write!(stdout, 
                                    "{:<6}  {:<30}  {}{:>6.1}{}  {}{:<10}{}\r\n",
                                    pid,
                                    command_display,
                                    cpu_color, cpu, reset,
                                    status_color, status, reset
                                ).unwrap();
                            }
                        }
                        
                        // If there are more children than we're showing
                        let child_count = child_pids.len() - 1; // Subtract 1 for parent
                        if child_count > 10 {
                            write!(stdout, "... and {} more child processes\r\n", child_count - 10).unwrap();
                        }
                    }
                }
            }
        }
        
        // Add spacing after the table
        write!(stdout, "\r\n").unwrap();
        
        // Print system stats
        // Print system stats
            let mem_percent = (system.used_memory() as f64 / system.total_memory() as f64) * 100.0;
            let mem_color = if mem_percent > 80.0 {
                high_usage_color
            } else if mem_percent > 50.0 {
                medium_usage_color
            } else {
                reset
            };

            let total_cpu: f32 = system.processes().values().map(|p| p.cpu_usage()).sum();
            let cpu_per_core = total_cpu / system.cpus().len() as f32;
            let cpu_color = if total_cpu > 80.0 {
                high_usage_color
            } else if total_cpu > 50.0 {
                medium_usage_color
            } else {
                reset
            };

            // Now display per-core usage (you can also change this to `total_cpu` if you prefer)
            write!(stdout, "{}System: {}{:.1}%{} CPU total | {}{:.1}%{} per core avg | {}{:.1}%{} Memory Used | {} Processes\r\n",
                header_color,
                cpu_color, total_cpu, reset,
                cpu_color, cpu_per_core, reset,
                mem_color, mem_percent, reset,
                system.processes().len()
            ).unwrap();


        
        // Display status message, if any
        if !status_message.is_empty() && status_timer > 0 {
            write!(stdout, "\r\n{}{}{}\r\n", bold, status_message, reset).unwrap();
            status_timer -= 1;
            if status_timer == 0 {
                status_message.clear();
            }
        }
        
        // Input mode indicator and help text
        match input_mode {
            InputMode::Normal => {
                write!(stdout, "\r\n{}{}Controls:{} ", help_color, bold, reset).unwrap();
                write!(stdout, "{}q{}: Quit | ", help_color, reset).unwrap();
                write!(stdout, "{}c{}: Sort by CPU | ", help_color, reset).unwrap();
                write!(stdout, "{}m{}: Sort by Memory | ", help_color, reset).unwrap();
                write!(stdout, "{}p{}: Sort by PID | ", help_color, reset).unwrap();
                write!(stdout, "{}g{}: Sort by Group | ", help_color, reset).unwrap();
                write!(stdout, "{}/{}: Search | ", help_color, reset).unwrap();
                write!(stdout, "{}k{}: Kill | ", help_color, reset).unwrap();
                write!(stdout, "{}r{}: Restart | ", help_color, reset).unwrap();
                write!(stdout, "{}s{}: Pause/Resume | ", help_color, reset).unwrap();
                write!(stdout, "{}v{}: View Groups | ", help_color, reset).unwrap();
                write!(stdout, "{}n{}: Set priority", help_color, reset).unwrap();
            },
            InputMode::Search => {
                write!(stdout, "\r\n{}SEARCH MODE:{} {} (Press Enter to execute, Esc to cancel)\r\n", 
                    bold, reset, search_query
                ).unwrap();
            },
            InputMode::Kill => {
                write!(stdout, "\r\n{}KILL MODE:{} Enter PID to kill: {} (Press Enter to execute, Esc to cancel)\r\n", 
                    bold, reset, pid_input
                ).unwrap();
            },
            InputMode::Pause => {
                write!(stdout, "\r\n{}PAUSE/RESUME MODE:{} Enter PID to toggle: {} (Press Enter to execute, Esc to cancel)\r\n", 
                    bold, reset, pid_input
                ).unwrap();
            },
            InputMode::Restart => {
                write!(stdout, "\r\n{}RESTART MODE:{} Enter PID to restart: {} (Press Enter to execute, Esc to cancel)\r\n", 
                    bold, reset, pid_input
                ).unwrap();
            },
            InputMode::Priority => {
                write!(stdout, "\r\n{}PRIORITY MODE:{} Enter PID to set priority: {} (Press Enter to continue, Esc to cancel)\r\n", 
                    bold, reset, pid_input
                ).unwrap();
            },
            InputMode::GroupView => {
                write!(stdout, "\r\n{}GROUP VIEW MODE:{} ", bold, reset).unwrap();
                write!(stdout, "{}q{}: Back to Process List | ", help_color, reset).unwrap();
                write!(stdout, "{}Enter{}: Select Group | ", help_color, reset).unwrap();
                write!(stdout, "{}p{}: Pause/Resume Group | ", help_color, reset).unwrap();
                write!(stdout, "{}k{}: Kill Group", help_color, reset).unwrap();
            },
            InputMode::GroupAction => {
                write!(stdout, "\r\n{}GROUP ACTION MODE:{} Enter Group ID: {} (Press Enter to execute, Esc to cancel)\r\n", 
                    bold, reset, pid_input
                ).unwrap();
            },
        }
        
        // Show cursor in input modes
        match input_mode {
            InputMode::Search | InputMode::Kill | InputMode::Pause | InputMode::Restart | InputMode::Priority | InputMode::GroupAction => {
                write!(stdout, "{}", cursor::Show).unwrap();
            },
            _ => {
                write!(stdout, "{}", cursor::Hide).unwrap();
            }
        }
        
        stdout.flush().unwrap();
        
        // Read input with a timeout
        if let Some(Ok(key)) = input.next() {
            match input_mode {
                InputMode::Normal => {
                    match key {
                        Key::Char('q') => quit = true,
                        Key::Char('c') => sort_mode = SortMode::Cpu,
                        Key::Char('m') => sort_mode = SortMode::Memory,
                        Key::Char('p') => sort_mode = SortMode::Pid,
                        Key::Char('g') => sort_mode = SortMode::Group,
                        Key::Char('/') => {
                            input_mode = InputMode::Search;
                            search_query.clear();
                        },
                        Key::Char('k') => {
                            input_mode = InputMode::Kill;
                            pid_input.clear();
                        },
                        Key::Char('s') => {
                            input_mode = InputMode::Pause;
                            pid_input.clear();
                        },
                        Key::Char('r') => {
                            input_mode = InputMode::Restart;
                            pid_input.clear();
                        },
                        Key::Char('n') => {
                            input_mode = InputMode::Priority;
                            pid_input.clear();
                        },
                        Key::Char('v') => {
                            // Switch to group view
                            view_state = ViewState::GroupView;
                            selected_group_pid = None;
                            input_mode = InputMode::GroupView;
                        },
                        _ => {}
                    }
                },
                InputMode::Search => {
                    match key {
                        Key::Char('\n') => {
                            // Execute search
                            force_refresh = true;
                            input_mode = InputMode::Normal;
                        },
                        Key::Esc => {
                            // Cancel search
                            search_query.clear();
                            input_mode = InputMode::Normal;
                        },
                        Key::Char(c) => {
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
                        Key::Char('\n') => {
                            // Execute kill command
                            if let Ok(pid_val) = pid_input.trim().parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);

                                // Request password for privileged operation
                                let password = prompt_password();
                                
                                // Execute kill command with sudo
                                let status = Command::new("sudo")
                                    .args(["-S", "kill", "-9", &pid_val.to_string()])
                                    .stdin(std::process::Stdio::piped())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn()
                                    .and_then(|mut child| {
                                        if let Some(mut stdin) = child.stdin.take() {
                                            // Write password to stdin
                                            stdin.write_all(format!("{}\n", password).as_bytes())?;
                                        }
                                        child.wait()
                                    });
                                
                                match status {
                                    Ok(exit_status) if exit_status.success() => {
                                        status_message = format!("Process {} killed successfully", pid_val);
                                        // Also remove from paused processes if it was paused
                                        process_controller.remove_terminated_process(&pid);
                                    },
                                    _ => {
                                        status_message = format!("Failed to kill process {}", pid_val);
                                    }
                                }
                                
                                status_timer = 5; // Show message for 5 refresh cycles
                            }
                            
                            pid_input.clear();
                            input_mode = InputMode::Normal;
                        },
                        Key::Esc => {
                            // Cancel kill command
                            pid_input.clear();
                            input_mode = InputMode::Normal;
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
                        Key::Char('\n') => {
                            // Execute pause/resume command
                            if let Ok(pid_val) = pid_input.trim().parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);
                                
                                // Check if the process exists
                                if system.process(pid).is_some() {
                                    // Check if already paused
                                    if process_controller.is_paused(&pid) {
                                        // Resume the process
                                        if let Ok(action) = process_controller.toggle_process(&pid) {
                                            match action {
                                                ProcessAction::Resume => {
                                                    status_message = format!("Process {} resumed", pid_val);
                                                },
                                                _ => {}
                                            }
                                        } else {
                                            status_message = format!("Failed to resume process {}", pid_val);
                                        }
                                    } else {
                                        // Pause the process
                                        if let Ok(action) = process_controller.toggle_process(&pid) {
                                            match action {
                                                ProcessAction::Pause => {
                                                    status_message = format!("Process {} paused", pid_val);
                                                },
                                                _ => {}
                                            }
                                        } else {
                                            status_message = format!("Failed to pause process {}", pid_val);
                                        }
                                    }
                                } else {
                                    status_message = format!("Process {} not found", pid_val);
                                }
                                
                                status_timer = 5; // Show message for 5 refresh cycles
                            }
                            
                            pid_input.clear();
                            input_mode = InputMode::Normal;
                        },
                        Key::Esc => {
                            // Cancel pause command
                            pid_input.clear();
                            input_mode = InputMode::Normal;
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
                        Key::Char('\n') => {
                            // Execute restart command
                            if let Ok(pid_val) = pid_input.trim().parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);
                                
                                // Request password for privileged operation
                                let password = prompt_password();
                                
                                // Try to restart the process
                                let result = process_restarter.restart_process(pid);
                                
                                match result {
                                    RestartResult::Success => {
                                        status_message = format!("Process {} restarted successfully", pid_val);
                                    },
                                    RestartResult::NotRunning => {
                                        status_message = format!("Process {} is not running", pid_val);
                                    },
                                    RestartResult::NoExecutable => {
                                        status_message = format!("Could not determine executable for process {}", pid_val);
                                    },
                                    RestartResult::Failed => {
                                        status_message = format!("Failed to restart process {}", pid_val);
                                    },
                                    RestartResult::KillFailed => {
                                        status_message = format!("Failed to kill process {}", pid_val);
                                    },
                                    RestartResult::NotFound => {
                                        status_message = format!("Process {} not found", pid_val);
                                    },
                                    RestartResult::RestartFailed => {
                                        status_message = format!("Restart failed for process {}", pid_val);
                                    },
                                }
                                
                                
                                status_timer = 5; // Show message for 5 refresh cycles
                            }
                            
                            pid_input.clear();
                            input_mode = InputMode::Normal;
                        },
                        Key::Esc => {
                            // Cancel restart command
                            pid_input.clear();
                            input_mode = InputMode::Normal;
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
                InputMode::Priority => {
                    match key {
                        Key::Char('\n') => {
                            // Validate PID input
                            if let Ok(pid_val) = pid_input.trim().parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);
                                
                                // Check if the process exists
                                if system.process(pid).is_some() {
                                    // Get current priority
                                    let pid_value = pid.as_u32() as u32;
                                    let current_priority = unsafe {
                                        let prio = getpriority(PRIO_PROCESS, pid_value);
                                        if prio == -1 && nix::errno::errno() != 0 {
                                            0 // Default if can't get current priority
                                        } else {
                                            prio
                                        }
                                    };
                                    
                                    // Request new priority value
                                    write!(stdout, "\r\n{}Current priority for PID {} is {}. Enter new priority (-20 to 19, lower is higher priority): ",
                                        reset, pid_val, current_priority
                                    ).unwrap();
                                    stdout.flush().unwrap();
                                    
                                    // Reset the pid_input to collect the new value
                                    pid_input.clear();
                                    
                                    // Keep track of the pid for the next step
                                    let process_pid = pid;
                                    
                                    // Switch to a different state to collect priority value
                                    // For simplicity, reuse the same input_mode but process differently in the next iteration
                                    // In a real implementation, you might want a separate state for this
                                    input_mode = InputMode::Priority;
                                    
                                    // Get priority value from stdin
                                    let mut priority_input = String::new();
                                    let _ = std::io::stdin().read_line(&mut priority_input);
                                    
                                    if let Ok(new_priority) = priority_input.trim().parse::<i32>() {
                                        // Validate priority range
                                        if new_priority >= -20 && new_priority <= 19 {
                                            // Request password for privileged operation
                                            let password = prompt_password();
                                            
                                            // Set new priority with sudo
                                            let status = Command::new("sudo")
                                                .args(["-S", "renice", &new_priority.to_string(), "-p", &pid_val.to_string()])
                                                .stdin(std::process::Stdio::piped())
                                                .stdout(std::process::Stdio::null())
                                                .stderr(std::process::Stdio::null())
                                                .spawn()
                                                .and_then(|mut child| {
                                                    if let Some(mut stdin) = child.stdin.take() {
                                                        // Write password to stdin
                                                        stdin.write_all(format!("{}\n", password).as_bytes())?;
                                                    }
                                                    child.wait()
                                                });
                                            
                                            match status {
                                                Ok(exit_status) if exit_status.success() => {
                                                    status_message = format!("Priority for process {} changed to {}", pid_val, new_priority);
                                                },
                                                _ => {
                                                    status_message = format!("Failed to change priority for process {}", pid_val);
                                                }
                                            }
                                        } else {
                                            status_message = "Priority must be between -20 and 19".to_string();
                                        }
                                    } else {
                                        status_message = "Invalid priority value".to_string();
                                    }
                                    
                                    status_timer = 5; // Show message for 5 refresh cycles
                                } else {
                                    status_message = format!("Process {} not found", pid_val);
                                    status_timer = 5;
                                }
                            }
                            
                            input_mode = InputMode::Normal;
                            pid_input.clear();
                        },
                        Key::Esc => {
                            // Cancel priority change
                            pid_input.clear();
                            input_mode = InputMode::Normal;
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
                InputMode::GroupView => {
                    match key {
                        Key::Char('q') => {
                            // Switch back to process list view
                            view_state = ViewState::ProcessList;
                            input_mode = InputMode::Normal;
                        },
                        Key::Char('\n') => {
                            // Switch to group action mode for selection
                            input_mode = InputMode::GroupAction;
                            pid_input.clear();
                        },
                        Key::Char('p') => {
                            // Pause/Resume Group
                            if let Some(parent_pid) = selected_group_pid {
                                // Toggle group pause state
                                let is_paused = process_group_manager.is_group_paused(&system, parent_pid);
                                if is_paused {
                                    // Resume the group
                                    if process_group_manager.resume_group(&system, parent_pid) {
                                        status_message = format!("Process group {} resumed", parent_pid);
                                    } else {
                                        status_message = format!("Failed to resume process group {}", parent_pid);
                                    }
                                } else {
                                    // Pause the group
                                    if process_group_manager.pause_group(&system, parent_pid) {
                                        status_message = format!("Process group {} paused", parent_pid);
                                    } else {
                                        status_message = format!("Failed to pause process group {}", parent_pid);
                                    }
                                }
                                status_timer = 5;
                            } else {
                                status_message = "No process group selected. Press Enter to select a group first.".to_string();
                                status_timer = 5;
                            }
                        },
                        Key::Char('k') => {
                            // Kill Group
                            if let Some(parent_pid) = selected_group_pid {
                                // Request password for privileged operation
                                let password = prompt_password();
                                
                                // Get all processes in the group
                                let pids = process_group_manager.get_group_pids(&system, parent_pid);
                                
                                // Kill all processes in the group
                                let mut success = true;
                                for &pid in &pids {
                                    let pid_val = pid.as_u32();
                                    
                                    // Execute kill command with sudo
                                    let status = Command::new("sudo")
                                        .args(["-S", "kill", "-9", &pid_val.to_string()])
                                        .stdin(std::process::Stdio::piped())
                                        .stdout(std::process::Stdio::null())
                                        .stderr(std::process::Stdio::null())
                                        .spawn()
                                        .and_then(|mut child| {
                                            if let Some(mut stdin) = child.stdin.take() {
                                                // Write password to stdin
                                                stdin.write_all(format!("{}\n", password).as_bytes())?;
                                            }
                                            child.wait()
                                        });
                                    
                                    if status.is_err() || !status.unwrap().success() {
                                        success = false;
                                    }
                                    
                                    // Also remove from paused processes if it was paused
                                    process_controller.remove_terminated_process(&pid);
                                }
                                
                                if success {
                                    status_message = format!("Process group {} killed successfully", parent_pid);
                                } else {
                                    status_message = format!("Failed to kill some processes in group {}", parent_pid);
                                }
                                status_timer = 5;
                                
                                // Reset the selected group
                                selected_group_pid = None;
                            } else {
                                status_message = "No process group selected. Press Enter to select a group first.".to_string();
                                status_timer = 5;
                            }
                        },
                        _ => {}
                    }
                },
                InputMode::GroupAction => {
                    match key {
                        Key::Char('\n') => {
                            // Select the group
                            if let Ok(pid_val) = pid_input.trim().parse::<u32>() {
                                let pid = Pid::from(pid_val as usize);
                                
                                // Check if the parent process exists
                                if system.process(pid).is_some() {
                                    selected_group_pid = Some(pid);
                                    status_message = format!("Process group {} selected", pid_val);
                                } else {
                                    status_message = format!("Process {} not found", pid_val);
                                }
                                
                                status_timer = 5; // Show message for 5 refresh cycles
                            }
                            
                            pid_input.clear();
                            input_mode = InputMode::GroupView;
                        },
                        Key::Esc => {
                            // Cancel group selection
                            pid_input.clear();
                            input_mode = InputMode::GroupView;
                        },
                        Key::Char(c) if c.is_digit(10) => {
                            pid_input.push(c);
                        },
                        Key::Backspace => {    system.refresh_all();
                            pid_input.pop();
                        },
                        _ => {}
                    }
                },
            }
        }
        
        // Sleep to prevent consuming too much CPU
        if !force_refresh {
            thread::sleep(refresh_duration);
        } else {
            force_refresh = false;
        }

    }
    
    // Restore terminal state
    write!(stdout, "{}", cursor::Show).unwrap();
    stdout.flush().unwrap();
}
