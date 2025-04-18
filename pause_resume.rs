use std::process::Command;
use sysinfo::Pid;

/// Enum to represent possible process control actions
pub enum ProcessAction {
    Pause,
    Resume,
}

/// Struct to track processes that have been paused by our application
pub struct ProcessController {
    paused_processes: Vec<Pid>,
}

impl ProcessController {
    /// Create a new ProcessController
    pub fn new() -> Self {
        ProcessController {
            paused_processes: Vec::new(),
        }
    }

    /// Pause or resume a process using the SIGSTOP and SIGCONT signals
    pub fn control_process(&mut self, pid: Pid, action: ProcessAction) -> Result<(), String> {
        let signal = match action {
            ProcessAction::Pause => {
                // Add to paused list if not already there
                if !self.paused_processes.contains(&pid) {
                    self.paused_processes.push(pid);
                }
                "SIGSTOP"
            },
            ProcessAction::Resume => {
                // Remove from paused list
                self.paused_processes.retain(|&p| p != pid);
                "SIGCONT"
            }
        };

        // Use kill command with appropriate signal
        let result = Command::new("kill")
            .arg(format!("-{}", signal))
            .arg(pid.to_string())
            .status();

        match result {
            Ok(status) => {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("Failed to {} process {}. Process might not exist or you may not have permission.", 
                        match action {
                            ProcessAction::Pause => "pause",
                            ProcessAction::Resume => "resume",
                        }, pid))
                }
            },
            Err(e) => Err(format!("Error: {}", e)),
        }
    }

    /// Check if a process is paused by our application
    pub fn is_paused(&self, pid: &Pid) -> bool {
        self.paused_processes.contains(pid)
    }

    /// Get all processes that are currently paused by our application
    pub fn get_paused_processes(&self) -> Vec<Pid> {
        // Return a copy of the paused processes to avoid borrow issues
        self.paused_processes.clone()
    }
    
    /// Resume all paused processes
    pub fn resume_all(&mut self) {
        // Clone the list first to avoid borrow issues
        let paused_pids = self.paused_processes.clone();
        
        for pid in paused_pids {
            // Ignore errors here, we're just trying our best to clean up
            let _ = self.control_process(pid, ProcessAction::Resume);
        }
    }
}
