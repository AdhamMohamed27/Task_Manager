use std::collections::{HashMap, HashSet};
use sysinfo::{Process, Pid, System};
use crate::pause_resume::{ProcessController, ProcessAction};

/// Represents a process group with a parent and its children
pub struct ProcessGroup {
    pub parent_pid: Pid,
    pub parent_name: String,
    pub children: Vec<Pid>,
}

/// Manages process groups based on parent-child relationships
pub struct ProcessGroupManager {
    process_controller: ProcessController,
}

impl ProcessGroupManager {
    /// Create a new ProcessGroupManager
    pub fn new() -> Self {
        ProcessGroupManager {
            process_controller: ProcessController::new(),
        }
    }

    /// Build a map of all processes grouped by their parent PIDs
    pub fn build_process_groups(&self, system: &System) -> Vec<ProcessGroup> {
        let mut parent_map: HashMap<Pid, Vec<Pid>> = HashMap::new();
        let mut process_names: HashMap<Pid, String> = HashMap::new();
        
        // First pass: collect all processes and their names
        for (pid, process) in system.processes() {
            process_names.insert(*pid, process.name().to_string());
        }
        
        // Second pass: build parent-child relationships
        for (pid, process) in system.processes() {
            if let Some(ppid) = process.parent() {
                parent_map.entry(ppid).or_insert_with(Vec::new).push(*pid);
            } else {
                // Process with no parent (usually init processes)
                parent_map.entry(*pid).or_insert_with(Vec::new);
            }
        }
        
        // Build the process groups
        let mut groups = Vec::new();
        for (parent_pid, children) in parent_map {
            if let Some(parent_name) = process_names.get(&parent_pid) {
                groups.push(ProcessGroup {
                    parent_pid,
                    parent_name: parent_name.clone(),
                    children,
                });
            }
        }
        
        // Sort groups by parent PID
        groups.sort_by(|a, b| a.parent_pid.cmp(&b.parent_pid));
        
        groups
    }
    
    /// Get a flattened list of PIDs for a specific parent, including the parent itself
    pub fn get_group_pids(&self, system: &System, parent_pid: Pid) -> Vec<Pid> {
        let mut result = vec![parent_pid];
        let mut pids_to_check = vec![parent_pid];
        let mut checked_pids = HashSet::new();
        
        // Breadth-first search to find all children and their children
        while let Some(current_pid) = pids_to_check.pop() {
            if checked_pids.contains(&current_pid) {
                continue;
            }
            
            checked_pids.insert(current_pid);
            
            for (pid, process) in system.processes() {
                if let Some(ppid) = process.parent() {
                    if ppid == current_pid {
                        result.push(*pid);
                        pids_to_check.push(*pid);
                    }
                }
            }
        }
        
        result
    }
    
    /// Control (pause/resume) all processes in a group based on the parent PID
    pub fn control_group(&mut self, system: &System, parent_pid: Pid, action: ProcessAction) -> Result<usize, String> {
        let group_pids = self.get_group_pids(system, parent_pid);
        let mut success_count = 0;
        
        for pid in group_pids {
            // Skip if process doesn't exist anymore
            if system.process(pid).is_none() {
                continue;
            }
            
            match self.process_controller.control_process(pid, action.clone()) {
                Ok(_) => success_count += 1,
                Err(_) => {} // Continue with other processes even if one fails
            }
        }
        
        if success_count > 0 {
            Ok(success_count)
        } else {
            Err("Failed to control any process in the group".to_string())
        }
    }
    
    /// Check if a process group is paused (true if all processes are paused)
    pub fn is_group_paused(&self, system: &System, parent_pid: Pid) -> bool {
        let group_pids = self.get_group_pids(system, parent_pid);
        let mut all_paused = true;
        let mut any_process_exists = false;
        
        for pid in group_pids {
            if let Some(_) = system.process(pid) {
                any_process_exists = true;
                if !self.process_controller.is_paused(&pid) {
                    all_paused = false;
                    break;
                }
            }
        }
        
        any_process_exists && all_paused
    }
    
    /// Get all currently paused processes (delegating to ProcessController)
    pub fn get_paused_processes(&self) -> Vec<Pid> {
        self.process_controller.get_paused_processes().clone()
    }
    
    /// Remove terminated processes from the paused list
    pub fn remove_terminated_process(&mut self, pid: &Pid) {
        self.process_controller.remove_terminated_process(pid);
    }

        /// Resume all processes in a group
        pub fn resume_group(&mut self, system: &System, parent_pid: Pid) -> bool {
            self.control_group(system, parent_pid, ProcessAction::Resume).is_ok()
        }
    
        /// Pause all processes in a group
        pub fn pause_group(&mut self, system: &System, parent_pid: Pid) -> bool {
            self.control_group(system, parent_pid, ProcessAction::Pause).is_ok()
        }
    
}
