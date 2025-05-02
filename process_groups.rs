use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use sysinfo::{ Pid, System};
use crate::pause_resume::{ProcessController, ProcessAction};

/// Represents a process group with a parent and its children in a tree structure
#[derive(Clone)]
pub struct ProcessNode {
    pub pid: Pid,
    pub name: String,
    pub children: Vec<ProcessNode>,
}

/// Represents a process group with a parent and its children
pub struct ProcessGroup {
    pub parent_pid: Pid,
    pub parent_name: String,
    pub children: Vec<Pid>,
}

/// Manages process groups based on parent-child relationships with caching
pub struct ProcessGroupManager {
    process_controller: ProcessController,
    process_tree_cache: Vec<ProcessNode>,
    flat_groups_cache: Vec<ProcessGroup>,
    last_update: Instant,
    update_interval: Duration,
}

impl ProcessGroupManager {
    /// Create a new ProcessGroupManager with default 5-second cache refresh
    pub fn new() -> Self {
        ProcessGroupManager {
            process_controller: ProcessController::new(),
            process_tree_cache: Vec::new(),
            flat_groups_cache: Vec::new(),
            last_update: Instant::now().checked_sub(Duration::from_secs(10)).unwrap_or(Instant::now()),
            update_interval: Duration::from_secs(5),
        }
    }

    /// Create with custom update interval
    pub fn with_update_interval(update_interval_secs: u64) -> Self {
        ProcessGroupManager {
            process_controller: ProcessController::new(),
            process_tree_cache: Vec::new(),
            flat_groups_cache: Vec::new(),
            last_update: Instant::now().checked_sub(Duration::from_secs(10)).unwrap_or(Instant::now()),
            update_interval: Duration::from_secs(update_interval_secs),
        }
    }

    /// Force rebuild the process tree cache
    pub fn force_update(&mut self, system: &System) {
        self.rebuild_process_tree(system);
        self.last_update = Instant::now();
    }

    /// Build a hierarchical tree of processes
    fn rebuild_process_tree(&mut self, system: &System) {
        // Map to store process information by PID
        let mut process_map: HashMap<Pid, (String, Vec<Pid>)> = HashMap::new();
        
        // First pass: collect all processes and their names
        for (pid, process) in system.processes() {
            process_map.insert(*pid, (process.name().to_string(), Vec::new()));
        }
        
        // Second pass: build parent-child relationships
        for (pid, process) in system.processes() {
            if let Some(ppid) = process.parent() {
                if let Some((_, children)) = process_map.get_mut(&ppid) {
                    children.push(*pid);
                }
            }
        }
        
        // Build tree structure starting with root processes (those with no parent or parent not in our list)
        let mut root_processes = Vec::new();
        
        for (pid, process) in system.processes() {
            let is_root = match process.parent() {
                Some(ppid) => !process_map.contains_key(&ppid),
                None => true,
            };
            
            if is_root {
                root_processes.push(*pid);
            }
        }
        
        // Sort root processes by PID
        root_processes.sort();
        
        // Recursively build the tree
        self.process_tree_cache = root_processes
            .into_iter()
            .filter_map(|pid| self.build_process_node(pid, &process_map))
            .collect();
            
        // Also update the flat group cache for backward compatibility
        self.rebuild_flat_groups(system);
    }
    
    // Helper method to build a process node recursively
    fn build_process_node(&self, pid: Pid, process_map: &HashMap<Pid, (String, Vec<Pid>)>) -> Option<ProcessNode> {
        match process_map.get(&pid) {
            Some((name, child_pids)) => {
                let mut children = Vec::new();
                let mut sorted_child_pids = child_pids.clone();
                sorted_child_pids.sort();
                
                for child_pid in sorted_child_pids {
                    if let Some(child_node) = self.build_process_node(child_pid, process_map) {
                        children.push(child_node);
                    }
                }
                
                Some(ProcessNode {
                    pid,
                    name: name.clone(),
                    children,
                })
            },
            None => None,
        }
    }
    
    // Rebuild flat groups (for backward compatibility)
    fn rebuild_flat_groups(&mut self, system: &System) {
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
        
        self.flat_groups_cache = groups;
    }

    /// Get the process tree, updating it only if the cache is stale
    pub fn get_process_tree(&mut self, system: &System) -> &Vec<ProcessNode> {
        if self.last_update.elapsed() >= self.update_interval {
            self.rebuild_process_tree(system);
            self.last_update = Instant::now();
        }
        &self.process_tree_cache
    }

    /// Build a map of all processes grouped by their parent PIDs (uses cache)
    pub fn build_process_groups(&mut self, system: &System) -> &Vec<ProcessGroup> {
        if self.last_update.elapsed() >= self.update_interval {
            self.rebuild_process_tree(system);
            self.last_update = Instant::now();
        }
        &self.flat_groups_cache
    }
    
    /// Format and print the process tree in a Linux-like tree view
    pub fn format_process_tree(&self) -> String {
        let mut result = String::new();
        for (i, node) in self.process_tree_cache.iter().enumerate() {
            let is_last = i == self.process_tree_cache.len() - 1;
            self.format_node(&mut result, node, "", is_last);
        }
        result
    }
    
    fn format_node(&self, result: &mut String, node: &ProcessNode, prefix: &str, is_last: bool) {
        // Add current node
        let pid_str = node.pid.to_string();
        
        if prefix.is_empty() {
            // Root level
            result.push_str(&format!("{}─ {} ({})\n", if is_last { "└" } else { "├" }, node.name, pid_str));
        } else {
            result.push_str(&format!("{}{}─ {} ({})\n", 
                prefix, 
                if is_last { "└" } else { "├" }, 
                node.name, 
                pid_str
            ));
        }
        
        // Process children
        let new_prefix = if prefix.is_empty() {
            if is_last { "  " } else { "│ " }.to_string()
        } else {
            format!("{}{}", prefix, if is_last { "  " } else { "│ " })
        };
        
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == node.children.len() - 1;
            self.format_node(result, child, &new_prefix, child_is_last);
        }
    }
    
    /// Get a flattened list of PIDs for a specific parent, including the parent itself
    /// Uses cached data when available
    pub fn get_group_pids(&mut self, system: &System, parent_pid: Pid) -> Vec<Pid> {
        // Ensure cache is up to date
        if self.last_update.elapsed() >= self.update_interval {
            self.rebuild_process_tree(system);
            self.last_update = Instant::now();
        }
        
        // Try to find the process node in the cache first
        if let Some(pids) = self.find_pids_in_tree(parent_pid) {
            return pids;
        }
        
        // Fallback to direct calculation if not in cache
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
    
    // Helper to find PIDs in the cached tree
    fn find_pids_in_tree(&self, target_pid: Pid) -> Option<Vec<Pid>> {
        // Create a function to search for the node with the target PID
        fn find_node<'a>(nodes: &'a [ProcessNode], target_pid: Pid) -> Option<&'a ProcessNode> {
            for node in nodes {
                if node.pid == target_pid {
                    return Some(node);
                }
                
                if let Some(found) = find_node(&node.children, target_pid) {
                    return Some(found);
                }
            }
            None
        }
        
        // Find the node with the target PID
        let target_node = find_node(&self.process_tree_cache, target_pid)?;
        
        // Collect all PIDs in this subtree
        let mut pids = Vec::new();
        
        fn collect_pids(node: &ProcessNode, pids: &mut Vec<Pid>) {
            pids.push(node.pid);
            for child in &node.children {
                collect_pids(child, pids);
            }
        }
        
        collect_pids(target_node, &mut pids);
        Some(pids)
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
    pub fn is_group_paused(&mut self, system: &System, parent_pid: Pid) -> bool {
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

    /// Toggle pause/resume for a process group
    pub fn toggle_process_group(&mut self, system: &System, parent_pid: Pid) -> Result<String, String> {
        if self.is_group_paused(system, parent_pid) {
            if self.resume_group(system, parent_pid) {
                Ok(format!("Resumed group of PID {}", parent_pid))
            } else {
                Err(format!("Failed to resume group of PID {}", parent_pid))
            }
        } else {
            if self.pause_group(system, parent_pid) {
                Ok(format!("Paused group of PID {}", parent_pid))
            } else {
                Err(format!("Failed to pause group of PID {}", parent_pid))
            }
        }
    }

}
