use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

pub fn append_record(path: impl AsRef<Path>, record: &str) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(record.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}
