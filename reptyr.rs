use std::process::Command;
use sysinfo::Pid;

/// Attach a running process (by PID) into the current terminal
/// using the external `reptyr` helper tool.
pub fn attach_to_terminal(pid: Pid) -> Result<(), String> {
    let status = Command::new("reptyr")
        .arg(pid.to_string())
        .status()
        .map_err(|e| format!("Failed to launch reptyr: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("reptyr exited with status: {}", status))
    }
}
