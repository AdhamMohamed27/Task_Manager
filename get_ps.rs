
use std::env;
use sysinfo::{ProcessStatus, System};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut show_header = true;

    // Check for --no-header or -n flag
    for arg in &args {
        if arg == "--no-header" || arg == "-n" {
            show_header = false;
        }
    }

    let mut system = System::new_all();
    system.refresh_all();

    if show_header {
        println!("{:<8} {:<10} {:<10} {:<12} {:<30}",
            "PID", "CPU%", "MEM (MB)", "STATE", "COMMAND");
        println!("{:-<80}", "");
    }

    let mut processes: Vec<_> = system.processes().iter().collect();
    processes.sort_by_key(|&(pid, _)| pid);

    for (&pid, process) in processes {
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

        let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
        let cmd = process.cmd().join(" ");
        let display_cmd = if cmd.is_empty() {
            process.name().to_string()
        } else {
            cmd
        };

        println!(
            "{:<8} {:<10.1} {:<10.1} {:<12} {:.30}",
            pid,
            process.cpu_usage(),
            memory_mb,
            state,
            display_cmd
        );
    }
}


// use std::env;
// use sysinfo::{ProcessStatus, System};


// fn main() {
//     let args: Vec<String> = env::args().collect();
//     let mut show_header = true;

//     for arg in &args {
//         if arg == "--no-header" || arg == "-n" {
//             show_header = false;
//         }
//     }

//     let mut system = System::new_all();
//     system.refresh_all();

//     if show_header {
//         println!(
//             "{:<10} {:<6} {:>5} {:>5} {:>8} {:>8} {:<6} {:<5} {:<8} {:<6} {:<}",
//             "USER", "PID", "%CPU", "%MEM", "VSZ", "RSS", "TTY", "STAT", "START", "TIME", "COMMAND"
//         );
//     }

//     let processes: Vec<_> = system.processes().iter().collect();

//     for (&pid, process) in processes {
//         let user = process.user_id().map_or("unknown".to_string(), |uid| uid.to_string());
//         let cpu = process.cpu_usage();
//         let memory_kb = process.memory(); // RSS in KB
//         let virtual_memory_kb = process.virtual_memory(); // VSZ in KB

//         let percent_mem = (memory_kb as f64 / system.total_memory() as f64) * 100.0;

//         let state = match process.status() {
//             ProcessStatus::Run => "R",
//             ProcessStatus::Sleep => "S",
//             ProcessStatus::Idle => "I",
//             ProcessStatus::Zombie => "Z",
//             ProcessStatus::Stop => "T",
//             _ => "?",
//         };

//         // TTY, START, TIME are placeholders
//         let tty = "?";
//         let start = "?";
//         let time = "?";

//         let cmd = process.cmd().join(" ");
//         let command = if cmd.is_empty() {
//             process.name().to_string()
//         } else {
//             cmd
//         };

//         println!(
//             "{:<10} {:<6} {:>5.1} {:>5.1} {:>8} {:>8} {:<6} {:<5} {:<8} {:<6} {}",
//             user,
//             pid,
//             cpu,
//             percent_mem,
//             virtual_memory_kb,
//             memory_kb,
//             tty,
//             state,
//             start,
//             time,
//             command
//         );
//     }
// }
