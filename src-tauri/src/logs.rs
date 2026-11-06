//! Reading and appending the plain-text logs in the CHAMP logs folder.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// How much of a log's end is read. Logs can grow far beyond what needs reading.
pub(crate) const LOG_TAIL_BYTES: u64 = 64 * 1024;

/// Best-effort append: a log that cannot be written must not fail the operation.
pub(crate) fn append_line(log_path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Read the last lines of a log file without loading the whole file. Lossy, since
/// logs can carry paths in the system code page.
pub(crate) fn read_tail_lines(path: &Path) -> Option<Vec<String>> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((len - start) as usize);
    file.read_to_end(&mut bytes).ok()?;

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if start > 0 && !lines.is_empty() {
        // The first line is probably cut in half.
        lines.remove(0);
    }
    Some(lines)
}
