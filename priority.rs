use libc::{getpriority, PRIO_PROCESS};
use nix::errno::Errno;
use libc::setpriority;

/// Change this process’s nice value.  
/// Returns Ok(msg) on success or Err(errmsg) on failure.
pub fn set_priority(pid: i32, nice: i32) -> Result<String, String> {
    // SAFETY: setpriority is a simple libc call
    let ret = unsafe { setpriority(PRIO_PROCESS, pid as u32, nice) };
    if ret < 0 {
        let e = Errno::last();
        Err(format!("{} (errno {})", e.desc(), e as i32))
    } else {
        Ok(format!("Nice set to {:+} for PID {}", nice, pid))
    }
}

pub fn get_nice_value(pid: i32) -> Result<i32, String> {
    // SAFETY: getpriority is a simple libc call
    let ret = unsafe { getpriority(PRIO_PROCESS, pid as u32) };
    if ret < 0 {
        let e = Errno::last();
        Err(format!("{} (errno {})", e.desc(), e as i32))
    } else {
        Ok(ret)
    }
}


// Layer	Numeric Range	Highest Priority	Lowest Priority
// Nice (NI)	-20 to +19	-20 (most CPU share)	+19 (least CPU share)
// Kernel PR (normal)	100-139	100 (when NI=-20)	139 (when NI=+19)
// Kernel PR (real-time)	0-99	0 (highest RT)	99 (lowest RT)
