# Pulse Linux Process Manager

In this project, we have developed an advanced Linux process manager named "Pulse" that enhances user experience and offers comprehensive process management features. 


# Implemented Functionalities

1. Sorting by CPU Usage: Facilitating quick identification of CPU-intensive processes.

2. Sorting by Memory Usage: Identifying memory-intensive processes for optimization.

3. Sorting by PID: Quickly organizing processes to aid direct management.

4. Searching by PID: Enabling users to rapidly locate and manage specific processes.

5. Pausing Processes: Temporarily halting processes, providing greater user control.

6. Resuming Processes: Allowing previously paused tasks to continue seamlessly.

7. Restarting Processes: Aiding users in resolving process-related issues without full termination.

8. Killing Processes: Effectively terminating unwanted processes to free resources.

9. Displaying Process Tree: Introducing a hierarchical, interactive view for intuitive process management:

10. Traversing the entire process hierarchy.

11. Interacting directly with processes within the tree.

12. Group Pause Operation: Allowing simultaneous pausing of related processes, enhancing user productivity.

13. Nice (Priority Adjustment): Providing a mechanism to adjust process priority, optimizing resource allocation dynamically.

14. Exporting to JSON: Enabling structured data export for external analysis and record-keeping.

15. Exporting to CSV: Facilitating data exports to spreadsheets and other analytical tools.

16. Real-time CPU and Memory Graphing: Visualizing the CPU and memory usage of specific processes over the past 25 seconds, offering immediate insights into performance.

# User Manual

These are the commands you can use in Pulse to interact with processes:

Q: Quit the application.

C: Sort processes by CPU usage.

M: Sort processes by memory usage.

P: Sort processes by PID.

S: Search for a process by PID.

K: Kill a process.

Z: Pause or resume a process.

R: Restart a process.

N: Set the priority (nice) value for a process.

G: Group pause operation on a set of processes.

T: Show the process tree.

J: Export processes as a JSON file.

E: Export processes as a CSV file.

H: Display the help screen.

V: Graph the CPU and memory usage for a specific process.

# Tree View Navigation:

Pulse provides a process tree that allows users to visually navigate through all processes.

↑ / ↓: Navigate through the process tree.

Enter: Select a process to kill. 

Esc: Exit the tree view.

