use std::process::{Command, Stdio};
use sysinfo::{Pid, System};
use std::{thread, time::Duration, fs, env};
use std::path::Path;

pub enum RestartResult {
    Success,
    KillFailed,
    NotFound,
    RestartFailed,
    NotRunning,
    NoExecutable,
    Failed,
}

pub struct ProcessRestarter {
    system: System,
}

impl ProcessRestarter {
    pub fn new() -> Self {
        ProcessRestarter {
            system: System::new_all(),
        }
    }

    pub fn restart_process(&mut self, pid: Pid) -> RestartResult {
        // Refresh the system info to get the latest process data
        self.system.refresh_all();
        
        // Check if process exists and get a snapshot of its details
        let process = match self.system.process(pid) {
            Some(p) => p,
            None => return RestartResult::NotFound,
        };

        if process.status().to_string() != "Run" {
            return RestartResult::NotRunning;
        }


        
        // Store process details before killing it
        let name = process.name().to_string();
        let exe = process.exe().map(|p| p.to_path_buf());
        let cwd = process.cwd().map(|p| p.to_path_buf());
        let parent_pid = process.parent();
        
        // Determine if this is a GUI application vs terminal process
        let is_gui_app = name.ends_with("-bin") || 
                      ["brave", "firefox", "chrome", "chromium", "electron"].iter()
                          .any(|app| name.contains(app));
        
        // Determine if this is a system service
        let is_system_service = name == "pulse" || name == "pulseaudio" || 
                               name.contains("daemon") || name.contains("service") || 
                               name.contains("systemd") || self.is_system_service(&name);
        
        // Special handling for VS Code
        let is_vscode = name == "code" || name.contains("code-oss") || name.contains("vscode");
        
        // Special handling for self (pulse program)
        let is_self = name == "pulse" && exe.as_ref().map_or(false, |p| p.to_string_lossy().contains("process_manager"));
        
        // Get command line arguments
        let cmdline = self.get_cmdline(pid).unwrap_or_else(|| {
            if let Some(exe_path) = &exe {
                vec![exe_path.to_string_lossy().to_string()]
            } else {
                vec![name.clone()]
            }
        });
        
        // Try to get environment variables for the process
        let env_vars = self.get_environ(pid);
        
        // For terminal processes, try to track the terminal
        let terminal_pid = if !is_gui_app && !is_system_service && parent_pid.is_some() {
            // Try to find the terminal by looking up the process tree
            self.find_terminal_in_hierarchy(parent_pid.unwrap())
        } else {
            None
        };
        
        // Special handling for self-restart (pulse)
        if is_self {
            // Create a self-restart script in /tmp
            let restart_script = "/tmp/restart_pulse.sh";
            let exe_path_str = exe.as_ref().map_or("pulse".to_string(), |p| p.to_string_lossy().to_string());
            let cwd_path_str = cwd.as_ref().map_or("/".to_string(), |p| p.to_string_lossy().to_string());
            
            // Create script content
            let script_content = format!(
                "#!/bin/bash\n\
                # Kill the current pulse process\n\
                kill {}\n\
                # Wait for termination\n\
                sleep 1\n\
                # Start a new instance\n\
                cd {} && {} &\n\
                exit 0",
                pid, cwd_path_str, exe_path_str
            );
            
            // Write script to file
            if let Ok(_) = fs::write(restart_script, script_content) {
                // Make executable
                let _ = Command::new("chmod").args(&["+x", restart_script]).output();
                
                // Execute script
                if let Ok(_) = Command::new("bash").arg(restart_script).spawn() {
                    return RestartResult::Success;
                }
            }
            
            // Fallback: fork and exec approach
            unsafe {
                match libc::fork() {
                    -1 => return RestartResult::RestartFailed,
                    0 => {
                        // Child process
                        thread::sleep(Duration::from_millis(500));
                        // Kill parent
                        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
                        thread::sleep(Duration::from_millis(500));
                        // Start new process
                        let _ = Command::new(&exe_path_str)
                            .current_dir(cwd.unwrap_or_else(|| Path::new("/").to_path_buf()))
                            .spawn();
                        std::process::exit(0);
                    },
                    _ => {
                        // Parent process - return success and let child handle it
                        return RestartResult::Success;
                    }
                }
            }
        }
        
        // Special handling for VS Code
        if is_vscode {
            // Find all related VS Code processes
            self.system.refresh_all();
            let related_pids: Vec<Pid> = self.system.processes()
                .iter()
                .filter(|(_, proc)| {
                    let proc_name = proc.name();
                    proc_name.contains("code") || proc_name.contains("vscode")
                })
                .map(|(pid, _)| *pid)
                .collect();
            
            // Kill all related processes, starting with child processes
            let mut sorted_pids = related_pids.clone();
            sorted_pids.sort_by(|a, b| b.cmp(a)); // Reverse sort to get child processes first
            
            for pid_to_kill in sorted_pids {
                let _ = Command::new("kill").arg("-15").arg(pid_to_kill.to_string()).output();
            }
            
            // Wait longer for VS Code to fully terminate
            thread::sleep(Duration::from_secs(1));
            
            // Use original command line to restart VS Code
            if !cmdline.is_empty() {
                match self.spawn_with_cmdline(&cmdline, cwd.as_deref(), env_vars.as_deref()) {
                    Ok(_) => return RestartResult::Success,
                    Err(_) => {}
                }
            }
            
            // Fallback: try executable path
            if let Some(path) = exe.as_deref() {
                match self.spawn_with_exe(path, cwd.as_deref()) {
                    Ok(_) => return RestartResult::Success,
                    Err(_) => {}
                }
            }
            
            // Last resort: try known VS Code commands
            for vscode_cmd in &["code", "code-oss", "vscode", "vscodium"] {
                match Command::new(vscode_cmd)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn() {
                    Ok(_) => return RestartResult::Success,
                    Err(_) => continue,
                }
            }
            
            return RestartResult::RestartFailed;
        }
        
        // For system services, we may not want to kill them directly
        if !is_system_service {
            // Kill process with proper cleanup handling
            if !self.kill_process(pid) {
                return RestartResult::KillFailed;
            }
            
            // Wait for process to fully terminate
            thread::sleep(Duration::from_millis(500));
        }
        
        // For system services: use specialized restart methods
        if is_system_service {
            if let Some(result) = self.restart_system_service(&name) {
                return result;
            }
            
            // If specialized restart failed and we haven't killed it yet, try killing and regular restart
            if !self.kill_process(pid) {
                return RestartResult::KillFailed;
            }
            
            thread::sleep(Duration::from_millis(500));
        }
        
        // For GUI apps: ensure old instance is gone before starting new one
        if is_gui_app {
            thread::sleep(Duration::from_millis(500));
            
            // Close any zombie instances with the same name
            self.cleanup_zombie_instances(&name);
            
            // Try different methods to restart the process
            if let Some(result) = self.try_restart_methods(
                &name, &cmdline, exe.as_deref(), cwd.as_deref(), env_vars.as_deref()
            ) {
                return result;
            }
        } else if let Some(term_pid) = terminal_pid {
            // For terminal processes: try to send a restart command to the terminal
            if let Some(result) = self.restart_in_terminal(
                term_pid, &name, &cmdline, cwd.as_deref()
            ) {
                return result;
            }
            
            // Fallback to standard methods if terminal-specific method fails
            if let Some(result) = self.try_restart_methods(
                &name, &cmdline, exe.as_deref(), cwd.as_deref(), env_vars.as_deref()
            ) {
                return result;
            }
        } else {
            // Generic restart attempt
            if let Some(result) = self.try_restart_methods(
                &name, &cmdline, exe.as_deref(), cwd.as_deref(), env_vars.as_deref()
            ) {
                return result;
            }
        }
        
        RestartResult::RestartFailed
    }

    // Helper to determine if a process is a system service
    fn is_system_service(&self, name: &str) -> bool {
        // Check if running as a systemd service
        if let Ok(output) = Command::new("systemctl")
            .args(["status", name])
            .output() {
            if output.status.success() {
                return true;
            }
        }
        
        // Check if running as a user systemd service
        if let Ok(output) = Command::new("systemctl")
            .args(["--user", "status", name])
            .output() {
            if output.status.success() {
                return true;
            }
        }
        
        false
    }

    // Specialized method for restarting system services
    fn restart_system_service(&self, name: &str) -> Option<RestartResult> {
        // For pulseaudio specifically
        if name == "pulse" || name == "pulseaudio" {
            // Try systemctl user restart first
            if let Ok(output) = Command::new("systemctl")
                .args(["--user", "restart", "pulseaudio"])
                .output() {
                if output.status.success() {
                    return Some(RestartResult::Success);
                }
            }
            
            // Try direct command restart as fallback
            if let Ok(_) = Command::new("pulseaudio")
                .args(["--kill"])
                .output() {
                
                thread::sleep(Duration::from_millis(500));
                
                // Now start it again
                if let Ok(_) = Command::new("pulseaudio")
                    .args(["--start"])
                    .spawn() {
                    return Some(RestartResult::Success);
                }
                
                // Last resort - try the pulse command
                if let Ok(_) = Command::new("pulse")
                    .args(["--start"])
                    .spawn() {
                    return Some(RestartResult::Success);
                }
            }
        }
        // For dbus-daemon, try dbus-launch
        else if name == "dbus-daemon" {
            if let Ok(_) = Command::new("dbus-launch")
                .spawn() {
                return Some(RestartResult::Success);
            }
        }
        // For system services running under systemd
        else if let Ok(output) = Command::new("systemctl")
            .args(["restart", name])
            .output() {
            if output.status.success() {
                return Some(RestartResult::Success);
            }
        }
        // For user systemd services
        else if let Ok(output) = Command::new("systemctl")
            .args(["--user", "restart", name])
            .output() {
            if output.status.success() {
                return Some(RestartResult::Success);
            }
        }
        // For services with "service" in the name, try both systemctl and service command
        else if name.contains("service") || name.contains("daemon") {
            // Try systemctl first
            if let Ok(output) = Command::new("systemctl")
                .args(["restart", name])
                .output() {
                if output.status.success() {
                    return Some(RestartResult::Success);
                }
            }
            
            // Then try the service command
            if let Ok(output) = Command::new("service")
                .args([name, "restart"])
                .output() {
                if output.status.success() {
                    return Some(RestartResult::Success);
                }
            }
        }
        
        None
    }

    // Helper method to kill a process with proper handling
    fn kill_process(&self, pid: Pid) -> bool {
        // Try SIGTERM first
        if let Ok(_) = Command::new("kill").arg(pid.to_string()).output() {
            // Wait for graceful termination
            for _ in 0..5 {
                thread::sleep(Duration::from_millis(300));
                if !self.process_exists(pid) {
                    return true;
                }
            }
            
            // If still running, use SIGKILL
            if let Ok(_) = Command::new("kill").arg("-9").arg(pid.to_string()).output() {
                for _ in 0..3 {
                    thread::sleep(Duration::from_millis(300));
                    if !self.process_exists(pid) {
                        return true;
                    }
                }
            }
        }
        
        !self.process_exists(pid)
    }

    // Try different restart methods and return the first successful one
    fn try_restart_methods(
        &self,
        name: &str,
        cmdline: &[String],
        exe_path: Option<&Path>,
        cwd: Option<&Path>,
        env_vars: Option<&[(String, String)]>
    ) -> Option<RestartResult> {
        // Method 1: Use command line if available
        if !cmdline.is_empty() {
            match self.spawn_with_cmdline(cmdline, cwd, env_vars) {
                Ok(_) => return Some(RestartResult::Success),
                Err(e) => eprintln!("Failed to restart using cmdline: {}", e),
            }
        }
        
        // Method 2: Use executable path
        if let Some(path) = exe_path {
            match self.spawn_with_exe(path, cwd) {
                Ok(_) => return Some(RestartResult::Success),
                Err(e) => eprintln!("Failed to restart using exe path: {}", e),
            }
        }
        
        // Method 3: Use process name as command
        match Command::new(name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn() {
            Ok(_) => return Some(RestartResult::Success),
            Err(_) => None,
        }
    }

    // Find the terminal in process hierarchy
    fn find_terminal_in_hierarchy(&self, start_pid: Pid) -> Option<Pid> {
        // List of common terminal process names
        const TERMINAL_NAMES: [&str; 7] = [
            "gnome-terminal", "konsole", "xterm", "alacritty", "kitty", "terminator", "tilix"
        ];
        
        let mut current_pid = start_pid;
        // Limit search depth to avoid infinite loops
        for _ in 0..10 {
            if let Some(process) = self.system.process(current_pid) {
                let name = process.name();
                if TERMINAL_NAMES.iter().any(|term| name.contains(term)) {
                    return Some(current_pid);
                }
                
                if let Some(parent) = process.parent() {
                    current_pid = parent;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        
        None
    }

    // Special handling for restarting processes in terminals
    fn restart_in_terminal(
        &self,
        term_pid: Pid,
        name: &str,
        cmdline: &[String],
        cwd: Option<&Path>
    ) -> Option<RestartResult> {
        // This is a simplified approach - a real implementation would need to
        // send commands to the terminal, which is complex and terminal-specific
        
        if let Some(term_proc) = self.system.process(term_pid) {
            let term_name = term_proc.name();
            
            // Construct a command string that would restart the process
            let cmd_str = if cmdline.len() > 1 {
                cmdline.join(" ")
            } else {
                name.to_string()
            };
            
            // Try to open new terminal with the command
            match Command::new(term_name)
                .arg("-e")
                .arg(&cmd_str)
                .current_dir(cwd.unwrap_or_else(|| Path::new("/")))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn() {
                Ok(_) => return Some(RestartResult::Success),
                Err(_) => {
                    // Try alternative terminal args format
                    match Command::new(term_name)
                        .args(&["-e", &cmd_str])
                        .current_dir(cwd.unwrap_or_else(|| Path::new("/")))
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn() {
                        Ok(_) => return Some(RestartResult::Success),
                        Err(_) => {}
                    }
                }
            }
        }
        
        None
    }

    // Clean up any zombie instances that might prevent proper restart
    fn cleanup_zombie_instances(&mut self, name: &str) {
        self.system.refresh_all();
        
        // Find all processes with the same name
        let zombie_pids: Vec<Pid> = self.system.processes()
            .iter()
            .filter(|(_, proc)| proc.name() == name)
            .map(|(pid, _)| *pid)
            .collect();
        
        // Try to kill any found instances
        for pid in zombie_pids {
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
        }
        
        // Small delay to let the system clean up
        thread::sleep(Duration::from_millis(300));
    }

    fn process_exists(&self, pid: Pid) -> bool {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_cmdline(&self, pid: Pid) -> Option<Vec<String>> {
        let path = format!("/proc/{}/cmdline", pid);
        match fs::read(path) {
            Ok(data) => {
                let parts: Vec<String> = data
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .collect();
                
                if parts.is_empty() {
                    None
                } else {
                    Some(parts)
                }
            },
            Err(_) => None
        }
    }
    
    fn get_environ(&self, pid: Pid) -> Option<Vec<(String, String)>> {
        let path = format!("/proc/{}/environ", pid);
        match fs::read(path) {
            Ok(data) => {
                let vars: Vec<(String, String)> = data
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| {
                        let var = String::from_utf8_lossy(s).to_string();
                        if let Some(pos) = var.find('=') {
                            Some((var[..pos].to_string(), var[pos+1..].to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();
                
                if vars.is_empty() {
                    None
                } else {
                    Some(vars)
                }
            },
            Err(_) => None
        }
    }

    fn spawn_with_cmdline(
        &self, 
        cmdline: &[String], 
        cwd: Option<&Path>,
        env_vars: Option<&[(String, String)]>
    ) -> Result<(), std::io::Error> {
        if cmdline.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput, 
                "Empty command line"
            ));
        }
        
        let program = &cmdline[0];
        let args = &cmdline[1..];
        
        let mut cmd = Command::new(program);
        cmd.args(args)
           .stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(Stdio::null());
        
        // Set working directory if available
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        
        // Set environment variables if available
        if let Some(vars) = env_vars {
            for (key, value) in vars {
                // Skip some variables that might cause issues when restarting
                if !["PPID", "PWD", "OLDPWD", "SHLVL", "_"].contains(&key.as_str()) {
                    cmd.env(key, value);
                }
            }
        }
        
        // Special handling for browser processes to avoid multiple instances
        if program.contains("brave") || program.contains("chrome") || program.contains("firefox") {
            // For browsers, add a small delay and ensure single-instance flags are set
            thread::sleep(Duration::from_millis(500));
            
            // Check if '--profile' is in args, if not, filter startup flags
            let has_profile = args.iter().any(|arg| arg.contains("--profile"));
            if !has_profile {
                // Filter out arguments that might prevent proper restart
                let filtered_args: Vec<&String> = args.iter()
                    .filter(|&arg| !arg.starts_with("--user-data-dir") && 
                                  !arg.starts_with("--no-startup") &&
                                  !arg.starts_with("--incognito"))
                    .collect();
                
                cmd = Command::new(program);
                cmd.args(filtered_args)
                   .stdin(Stdio::null())
                   .stdout(Stdio::null())
                   .stderr(Stdio::null());
                
                // Reset working directory
                if let Some(dir) = cwd {
                    cmd.current_dir(dir);
                }
                
                // Reset environment variables
                if let Some(vars) = env_vars {
                    for (key, value) in vars {
                        if !["PPID", "PWD", "OLDPWD", "SHLVL", "_"].contains(&key.as_str()) {
                            cmd.env(key, value);
                        }
                    }
                }
            }
        }
        
        // Try to spawn the process
        cmd.spawn()?;
        
        Ok(())
    }
    
    fn spawn_with_exe(&self, exe_path: &Path, cwd: Option<&Path>) -> Result<(), std::io::Error> {
        let mut cmd = Command::new(exe_path);
        
        cmd.stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(Stdio::null());
           
        // Set working directory if available
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        
        // Try current user's environment variables
        for (key, value) in env::vars() {
            cmd.env(key, value);
        }
        
        cmd.spawn()?;
        
        Ok(())
    }
}
