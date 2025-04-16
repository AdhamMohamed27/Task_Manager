use std::io::{stdout, Write, stdin};
use std::{thread, time::Duration};
use crossterm::{
    cursor,
    execute,
    terminal::{self, ClearType},
    style::Print,
    ExecutableCommand,
    event::{self, Event, KeyCode}
};
use sysinfo::{System, ProcessStatus, Pid};
use users::get_user_by_uid;

fn main() {
    let mut stdout = stdout();
    let mut system = System::new_all();

    // Enter alternate screen (clean full-screen UI)
    execute!(stdout, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    loop {
        system.refresh_processes();
        system.refresh_memory();
        system.refresh_cpu();

        // Move cursor to top
        execute!(
            stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::All)
        ).unwrap();

        writeln!(stdout, "Pulse – Linux Process Monitor\n").unwrap();
        writeln!(
            stdout,
            "{:<6} {:<12} {:>6} {:>6} {:<10} {:<31}",
            "PID", "USER", "CPU%", "MEM%", "STATE", "COMMAND"
        ).unwrap();
        writeln!(stdout, "{:-<84}", "").unwrap();

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

            writeln!(
                stdout,
                "{:<6} {:<12} {:>6.1} {:>6.1} {:<10} {:<31}",
                pid, username, cpu, mem, state, command_display
            ).unwrap();
        }
      
        if event::poll(Duration::from_millis(10)).unwrap(){
            if let Event::Key(key_event) = event::read().unwrap() {
                if key_event.code == KeyCode::Char('k') {
                  terminal::disable_raw_mode().unwrap();
                  print!("Enter the PID of the process you want to kill: ");
                  stdout().flush().unwrap();

                  let mut input = String::new();
                  stdin().read_line(&mut input).unwrap();

                  let pid: i32 = input.trim().parse().unwrap();
                  if let Some(process) = system.process(Pid::from(pid)) {
                    if process.kill() {
                      println!("Process of PID {} has been killed.", pid);
                    }
                    else {
                      println!("Oops, this process has not been killed :(");
                    }
                    system.refresh_processes();
                  }
                  terminal::enable_raw_mode().unwrap();
                }
            }
        }
      
        stdout.flush().unwrap();
        thread::sleep(Duration::from_millis(500));
    }

    // (unreachable for now) On exit: restore screen
    // execute!(stdout, terminal::LeaveAlternateScreen).unwrap();
    // terminal::disable_raw_mode().unwrap();
}
