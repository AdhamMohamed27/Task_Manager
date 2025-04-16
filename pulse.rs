use std::{thread, time::Duration};
use std::io::{stdout, Write, stdin};
use std::process::Command;
use sysinfo::{System, ProcessStatus, Pid};
use users::get_user_by_uid;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{cursor, clear};
use termion::async_stdin;
use signal_hook::consts::signal::SIGINT;
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Enum to track current sort mode
enum SortMode {
    Cpu,
    Memory,
    Pid,
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
    
    // Application state
    let mut sort_mode = SortMode::Cpu;
    let mut search_mode = false;
    let mut search_query = String::new();
    let mut search_results: Vec<Pid> = Vec::new();
    let mut quit = false;
    let mut kill_mode = false;
    let mut kill_pid_input = String::new();
    
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
        write!(stdout, "{:<6} {:<15} {:>6} {:>6} {:<10} {:<30}\r\n", 
            "PID", "USER", "CPU%", "MEM%", "STATE", "COMMAND"
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
        let display_processes = if search_mode && !search_query.is_empty() {
            processes.iter()
                .filter(|p| p.pid().to_string().contains(&search_query))
                .collect::<Vec<_>>()
        } else {
            processes.iter().collect()
        };
        
        // Calculate how many processes we can show
        let max_processes = height as usize - 8; // Account for header, footer, etc.
        
        // Display processes
        for process in display_processes.iter().take(max_processes) {
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
            
            // Color for state
            let state_color = match process.status() {
                ProcessStatus::Run => running_color,
                ProcessStatus::Sleep => sleep_color,
                ProcessStatus::Idle => idle_color,
                ProcessStatus::Stop => stopped_color,
                ProcessStatus::Zombie => zombie_color,
                _ => reset,
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
                "{:<6} {}{:<15}{} {}{:>6.1}{} {}{:>6.1}{} {}{:<10}{} {:<30}\r\n",
                pid, 
                user_color, username, reset,
                cpu_color, cpu, reset,
                mem_color, mem, reset,
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
        write!(stdout, "{}{}CPUs: {} cores, Processes: {}{}\r\n", 
            separator_color, bold, num_cores, display_processes.len(), reset
        ).unwrap();
        
        // Help line at bottom
        write!(stdout, "{}{}", help_color, bold).unwrap();
        if search_mode {
            write!(stdout, "Search PID: {} | Enter when Done | Esc to cancel", search_query).unwrap();
        } else if kill_mode {
            write!(stdout, "Kill PID: {} | Enter to confirm | Esc to cancel", kill_pid_input).unwrap();
        } else {
            // Updated key mappings in help text
            write!(stdout, "q:Quit | c:Sort CPU | m:Sort Mem | p:Sort PID | s:Search | k:Kill").unwrap();
        }
        write!(stdout, "{}", reset).unwrap();
        
        // Make sure to flush stdout to display updates
        stdout.flush().unwrap();
        
        // Handle input (non-blocking)
        if let Some(Ok(key)) = input.next() {
            if search_mode {
                match key {
                    Key::Esc => {
                        search_mode = false;
                        search_query.clear();
                    },
                    Key::Char('\n') => {
                        search_mode = false;
                    },
                    Key::Char(c) if c.is_digit(10) => {
                        search_query.push(c);
                    },
                    Key::Backspace => {
                        search_query.pop();
                    },
                    _ => {}
                }
            } else if kill_mode {
                match key {
                    Key::Esc => {
                        kill_mode = false;
                        kill_pid_input.clear();
                    },
                    Key::Char('\n') => {
                        if !kill_pid_input.is_empty() {
                            if let Ok(pid) = kill_pid_input.parse::<i32>() {
                                let result = Command::new("kill")
                                    .arg(pid.to_string())
                                    .output();
                                if result.is_err() {
                                    // Just clear screen and continue instead of adding text
                                    // which could cause duplication
                                }
                            }
                        }
                        kill_mode = false;
                        kill_pid_input.clear();
                    },
                    Key::Char(c) if c.is_digit(10) => {
                        kill_pid_input.push(c);
                    },
                    Key::Backspace => {
                        kill_pid_input.pop();
                    },
                    _ => {}
                }
            } else {
                match key {
                    // New key mappings
                    Key::Char('q') => quit = true,
                    Key::Char('c') => sort_mode = SortMode::Cpu,
                    Key::Char('m') => sort_mode = SortMode::Memory,
                    Key::Char('p') => sort_mode = SortMode::Pid,
                    Key::Char('s') => {
                        search_mode = true;
                        search_query.clear();
                    },
                    Key::Char('k') => {
                        kill_mode = true;
                        kill_pid_input.clear();
                    },
                    // Keep F-key mappings as well for backward compatibility
                    Key::F(1) => quit = true,
                    Key::F(2) => sort_mode = SortMode::Cpu,
                    Key::F(3) => sort_mode = SortMode::Memory,
                    Key::F(4) => sort_mode = SortMode::Pid,
                    Key::F(5) => {
                        search_mode = true;
                        search_query.clear();
                    },
                    Key::F(9) => {
                        kill_mode = true;
                        kill_pid_input.clear();
                    },
                    _ => {}
                }
            }
        }
        
        // Use a shorter sleep duration for more responsive updates
        thread::sleep(Duration::from_millis(300)); // Even shorter for better responsiveness
    }
    
    // Clean up terminal
    write!(stdout, "{}{}", cursor::Show, clear::All).unwrap();
    stdout.flush().unwrap();
}
