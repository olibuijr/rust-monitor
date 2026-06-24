use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

pub struct LogTailer {
    positions: HashMap<String, u64>,
}

impl LogTailer {
    pub fn new(paths: &[String]) -> Self {
        let mut positions = HashMap::new();
        for path in paths {
            // Seek to end on startup — only capture new lines
            if let Ok(meta) = std::fs::metadata(path) {
                positions.insert(path.clone(), meta.len());
            }
        }
        LogTailer { positions }
    }

    pub fn read_new_lines(&mut self) -> Vec<(String, String)> {
        let mut entries = Vec::new();

        let paths: Vec<String> = self.positions.keys().cloned().collect();
        for path in &paths {
            let pos = *self.positions.get(path).unwrap_or(&0);

            let Ok(mut file) = std::fs::File::open(path) else {
                continue;
            };

            let Ok(meta) = file.metadata() else {
                continue;
            };

            // File was truncated/rotated — reset to start
            let current_len = meta.len();
            let seek_pos = if current_len < pos { 0 } else { pos };

            if file.seek(SeekFrom::Start(seek_pos)).is_err() {
                continue;
            }

            let mut buf = String::new();
            if file.read_to_string(&mut buf).is_err() {
                continue;
            }

            for line in buf.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    entries.push((path.clone(), line.to_string()));
                }
            }

            self.positions.insert(path.clone(), current_len);
        }

        entries
    }
}
