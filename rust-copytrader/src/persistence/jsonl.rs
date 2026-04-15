use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLogKind {
    Activity,
    Orders,
    Verification,
}

impl SessionLogKind {
    pub const fn file_prefix(self) -> &'static str {
        match self {
            Self::Activity => "activity",
            Self::Orders => "orders",
            Self::Verification => "verification",
        }
    }
}

pub fn session_log_path(
    root: &Path,
    session_id: &str,
    kind: SessionLogKind,
    segment: u64,
) -> PathBuf {
    root.join("sessions")
        .join(session_id)
        .join("logs")
        .join(format!("{}-{segment:04}.jsonl", kind.file_prefix()))
}

#[derive(Debug, Clone)]
pub struct RotatingJsonlWriter {
    root: PathBuf,
    session_id: String,
    kind: SessionLogKind,
    max_records_per_file: usize,
    current_segment: u64,
    records_in_segment: usize,
}

impl RotatingJsonlWriter {
    pub fn new(
        root: impl Into<PathBuf>,
        session_id: impl Into<String>,
        kind: SessionLogKind,
        max_records_per_file: usize,
    ) -> Self {
        Self {
            root: root.into(),
            session_id: session_id.into(),
            kind,
            max_records_per_file: max_records_per_file.max(1),
            current_segment: 1,
            records_in_segment: 0,
        }
    }

    pub fn append(&mut self, record: &str) -> io::Result<PathBuf> {
        if self.records_in_segment >= self.max_records_per_file {
            self.current_segment += 1;
            self.records_in_segment = 0;
        }

        let path = session_log_path(
            &self.root,
            &self.session_id,
            self.kind,
            self.current_segment,
        );
        append_record(&path, record)?;
        self.records_in_segment += 1;
        Ok(path)
    }
}
