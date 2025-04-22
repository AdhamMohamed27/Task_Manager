use std::process::Command;

pub fn set_priority(pid: i32, prio: i32) -> Result<String, String> {
    let output = Command::new("renice")
        .arg(prio.to_string())
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to run renice: {}", e))?;

    if output.status.success() {
        Ok(format!("Priority set to {} for PID {}", prio, pid))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
