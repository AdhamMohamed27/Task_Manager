use std::process::Command;
use sysinfo::Pid;
use std::{thread, time::Duration};

pub enum RestartResult {
    Success,
    KillFailed,
    NotFound,
}

pub struct ProcessRestarter;

impl ProcessRestarter {
    pub fn new() -> Self {
        ProcessRestarter {}
    }

    // Restart a process by killing it and letting it respawn
    // (works for services and processes controlled by a supervisor/init system)
    pub fn restart_process(&self, pid: Pid) -> RestartResult {
        // Check if process exists
        if !self.process_exists(pid) {
            return RestartResult::NotFound;
        }

        // Send SIGTERM signal to gracefully kill the process
        match Command::new("kill")
            .arg(pid.to_string())
            .output() {
                Ok(_) => {
                    // Wait a short time to let the process terminate
                    thread::sleep(Duration::from_millis(500));
                    
                    // Check if process is still running
                    if self.process_exists(pid) {
                        // Process didn't terminate, try SIGKILL (-9)
                        match Command::new("kill")
                            .arg("-9")
                            .arg(pid.to_string())
                            .output() {
                                Ok(_) => {
                                    thread::sleep(Duration::from_millis(500));
                                    
                                    if self.process_exists(pid) {
                                        return RestartResult::KillFailed;
                                    }
                                    RestartResult::Success
                                },
                                Err(_) => RestartResult::KillFailed,
                            }
                    } else {
                        // Process terminated successfully
                        RestartResult::Success
                    }
                },
                Err(_) => RestartResult::KillFailed,
            }
    }

    // Check if a process with the given PID exists
    fn process_exists(&self, pid: Pid) -> bool {
        // Using kill with signal 0 to check if process exists without sending actual signal
        let output = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output();
        
        match output {
            Ok(result) => result.status.success(),
            Err(_) => false,
        }
    }
}
