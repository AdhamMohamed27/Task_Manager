use std::process::Command;
use sysinfo::{Pid, System};
use std::fs;

// Define actions that can be performed on processes
#[derive(Clone)]
pub enum ProcessAction {
    Pause,
    Resume,
}

pub struct ProcessController {
    paused_processes: Vec<Pid>,
}

impl ProcessController {
    pub fn new() -> Self {
        ProcessController {
            paused_processes: Vec::new(),
        }
    }

    pub fn is_paused(&self, pid: &Pid) -> bool {
        self.paused_processes.contains(pid)
    }

    pub fn get_paused_processes(&self) -> &Vec<Pid> {
        &self.paused_processes
    }

    pub fn remove_terminated_process(&mut self, pid: &Pid) {
        self.paused_processes.retain(|p| p != pid);
    }

    // Check if a process is a zombie
    fn is_zombie(pid: Pid) -> bool {
        let stat_path = format!("/proc/{}/stat", pid);
        match fs::read_to_string(&stat_path) {
            Ok(content) => {
                let parts: Vec<&str> = content.split_whitespace().collect();
                if let Some(state) = parts.get(2) {
                    return *state == "Z";
                }
                false
            },
            Err(_) => false,
        }
    }

    pub fn control_process(&mut self, pid: Pid, action: ProcessAction) -> Result<(), String> {
        if Self::is_zombie(pid) {
            return Err(format!("Process {} is a zombie and cannot be paused or resumed.", pid));
        }

        let signal = match action {
            ProcessAction::Pause => {
                if !self.paused_processes.contains(&pid) {
                    self.paused_processes.push(pid);
                }
                "SIGSTOP"
            },
            ProcessAction::Resume => {
                self.paused_processes.retain(|&p| p != pid);
                "SIGCONT"
            },
        };

        let result = Command::new("kill")
            .arg(format!("-{}", signal))
            .arg(pid.to_string())
            .status();

        match result {
            Ok(status) => {
                if status.success() {
                    Ok(())
                } else {
                    // Rollback state change
                    match action {
                        ProcessAction::Pause => {
                            self.paused_processes.retain(|&p| p != pid);
                        },
                        ProcessAction::Resume => {
                            if !self.paused_processes.contains(&pid) {
                                self.paused_processes.push(pid);
                            }
                        }
                    }
                    Err(format!(
                        "Failed to {} process {}. Process might not exist or you may not have permission.",
                        match action {
                            ProcessAction::Pause => "pause",
                            ProcessAction::Resume => "resume",
                        },
                        pid
                    ))
                }
            },
            Err(e) => {
                // Rollback state change
                match action {
                    ProcessAction::Pause => {
                        self.paused_processes.retain(|&p| p != pid);
                    },
                    ProcessAction::Resume => {
                        if !self.paused_processes.contains(&pid) {
                            self.paused_processes.push(pid);
                        }
                    }
                }
                Err(format!("Error: {}", e))
            },
        }
    }

    pub fn toggle_process(&mut self, pid: &Pid) -> Result<ProcessAction, String> {
        if self.is_paused(pid) {
            self.control_process(*pid, ProcessAction::Resume)?;
            Ok(ProcessAction::Resume)
        } else {
            self.control_process(*pid, ProcessAction::Pause)?;
            Ok(ProcessAction::Pause)
        }
    }

    pub fn resume_all(&mut self) {
        let paused_pids = self.paused_processes.clone();
        let mut pids_to_remove = Vec::new();

        for &pid in paused_pids.iter() {
            match self.control_process(pid, ProcessAction::Resume) {
                Ok(_) => {},
                Err(e) => {
                    if e.contains("might not exist") {
                        pids_to_remove.push(pid);
                    }
                }
            }
        }

        for pid in pids_to_remove {
            self.remove_terminated_process(&pid);
        }
    }
}
