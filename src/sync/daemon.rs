use std::fs;
use std::path::PathBuf;

use crate::error::{Result, SlackersError};

/// Return the path to the PID file: `~/.local/share/slackers/sync.pid`.
fn pid_file_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| SlackersError::Other("cannot determine data directory".into()))?;
    Ok(data_dir.join("slackers").join("sync.pid"))
}

/// Write the current process PID to the PID file.
pub fn write_pid_file() -> Result<()> {
    let path = pid_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let pid = std::process::id();
    fs::write(&path, pid.to_string())?;
    Ok(())
}

/// Remove the PID file if it exists.
pub fn remove_pid_file() -> Result<()> {
    let path = pid_file_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Read the PID from the PID file. Returns `None` if the file does not exist.
fn read_pid() -> Result<Option<u32>> {
    let path = pid_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&path)?;
    let pid: u32 = contents
        .trim()
        .parse()
        .map_err(|e| SlackersError::Store(format!("invalid PID in {}: {}", path.display(), e)))?;
    Ok(Some(pid))
}

/// Check whether the sync daemon is currently running.
///
/// Reads the PID file and verifies the process exists via `/proc/<pid>`.
pub fn is_running() -> bool {
    match read_pid() {
        Ok(Some(pid)) => process_exists(pid),
        _ => false,
    }
}

/// Stop the sync daemon by sending SIGTERM to the recorded PID.
pub fn stop_daemon() -> Result<()> {
    let pid = read_pid()?;
    match pid {
        Some(pid) => {
            if !process_exists(pid) {
                eprintln!("[sync] stale PID file (process {} not running), cleaning up", pid);
                remove_pid_file()?;
                return Ok(());
            }
            eprintln!("[sync] sending SIGTERM to PID {}", pid);
            // Safety: kill() with a valid signal is safe for any pid.
            let ret = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                return Err(SlackersError::Other(format!(
                    "failed to send SIGTERM to {}: {}",
                    pid, err
                )));
            }
            // Remove the PID file after successfully sending the signal.
            remove_pid_file()?;
            Ok(())
        }
        None => {
            eprintln!("[sync] no PID file found — daemon not running");
            Ok(())
        }
    }
}

/// Check whether a process with the given PID exists.
fn process_exists(pid: u32) -> bool {
    // Use /proc filesystem to check process existence.
    let proc_path = format!("/proc/{}", pid);
    std::path::Path::new(&proc_path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_file_path() {
        let path = pid_file_path().unwrap();
        assert!(path.to_string_lossy().contains("slackers"));
        assert!(path.to_string_lossy().ends_with("sync.pid"));
    }

    #[test]
    fn test_process_exists_self() {
        let pid = std::process::id();
        assert!(process_exists(pid));
    }

    #[test]
    fn test_process_exists_nonexistent() {
        // PID 4000000 is unlikely to exist.
        assert!(!process_exists(4_000_000));
    }
}
