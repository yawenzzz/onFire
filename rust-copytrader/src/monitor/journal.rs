use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MonitorJournal {
    dir: PathBuf,
    rotate_bytes: u64,
    keep_files: usize,
    current_index: usize,
}

impl MonitorJournal {
    pub fn new(dir: impl Into<PathBuf>, rotate_bytes: u64, keep_files: usize) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            rotate_bytes: rotate_bytes.max(1),
            keep_files: keep_files.max(1),
            current_index: 1,
        })
    }

    fn path(&self, index: usize) -> PathBuf {
        self.dir.join(format!("monitor-{index:02}.jsonl"))
    }

    fn rotate_if_needed(&mut self) -> io::Result<()> {
        let path = self.path(self.current_index);
        if !path.exists() {
            return Ok(());
        }
        let size = path.metadata()?.len();
        if size < self.rotate_bytes {
            return Ok(());
        }
        self.current_index += 1;
        if self.current_index > self.keep_files {
            self.current_index = 1;
        }
        fs::write(self.path(self.current_index), b"")?;
        Ok(())
    }

    pub fn append(&mut self, line: &str) -> io::Result<PathBuf> {
        self.rotate_if_needed()?;
        let path = self.path(self.current_index);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(path)
    }
}

pub fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub fn ensure_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MonitorJournal;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("monitor-journal-{name}-{suffix}"))
    }

    #[test]
    fn journal_rotates_files() {
        let root = unique_temp_dir("rotate");
        fs::create_dir_all(&root).expect("temp dir created");
        let mut journal = MonitorJournal::new(&root, 20, 2).expect("journal");
        journal.append("first line").expect("append");
        journal
            .append("second line that triggers rotation")
            .expect("append");
        let files = fs::read_dir(&root)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert!(!files.is_empty());
    }
}
