use std::fs;
use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};

/// Reads `/proc/<pid>/cmdline` (NUL-separated argv).
pub fn read_cmdline(pid: i32) -> Result<Vec<String>> {
    let path = Path::new("/proc").join(pid.to_string()).join("cmdline");
    let mut f = fs::File::open(&path).map_err(|e| Error::ProcCmdline { pid, source: e })?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| Error::ProcCmdline { pid, source: e })?;
    if buf.is_empty() {
        return Ok(Vec::new());
    }
    Ok(buf
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| String::from_utf8(s.to_vec()).ok())
        .collect())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::read_cmdline;

    #[test]
    fn read_cmdline_pid_1_non_empty() {
        let argv = read_cmdline(1).expect("read init cmdline");
        assert!(
            !argv.is_empty(),
            "PID 1 cmdline should be readable on Linux"
        );
    }
}
