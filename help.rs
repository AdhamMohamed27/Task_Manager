pub fn get_help_text() -> String {
    let mut help = String::new();
    use std::fmt::Write;

    writeln!(help, "{}{}Pulse - Linux Process Monitor Help{}\r\n", "\x1B[1m", "\x1B[38;5;82m", "\x1B[0m").unwrap();

    writeln!(help, "{}General Commands:{}\r", "\x1B[38;5;39m", "\x1B[0m").unwrap();
    writeln!(help, "  Q       Quit the application\r").unwrap();
    writeln!(help, "  C       Sort by CPU usage\r").unwrap();
    writeln!(help, "  M       Sort by Memory usage\r").unwrap();
    writeln!(help, "  P       Sort by PID\r").unwrap();
    writeln!(help, "  S       Search by PID\r").unwrap();
    writeln!(help, "  K       Kill a process\r").unwrap();
    writeln!(help, "  Z       Pause/Resume a process\r").unwrap();
    writeln!(help, "  R       Restart a process\r").unwrap();
    writeln!(help, "  N       Set nice value (priority)\r").unwrap();
    writeln!(help, "  G       Pause/Resume process group\r").unwrap();
    writeln!(help, "  T       Show process tree view\r").unwrap();
    writeln!(help, "  J       Export as JSON\r").unwrap();
    writeln!(help, "  E       Export as CSV\r").unwrap();
    writeln!(help, "  H       Show this help screen\r\n").unwrap();

    writeln!(help, "{}Tree View Navigation:{}\r", "\x1B[38;5;39m", "\x1B[0m").unwrap();
    writeln!(help, "  ↑ / ↓   Navigate process tree\r").unwrap();
    writeln!(help, "  Enter   Select a process for action\r").unwrap();
    writeln!(help, "  Esc     Exit tree view\r\n").unwrap();

    writeln!(
        help,
        "{}Pulse{} is a real-time Linux process monitor that lets you sort, search, manage,\r\n\
         and export process data. Use this interface to efficiently interact with running tasks.\r\n",
        "\x1B[38;5;147m", "\x1B[0m"
    )
    .unwrap();


    writeln!(help, "Press {}ESC{} to return.\r", "\x1B[1m", "\x1B[0m").unwrap();

    help
}
