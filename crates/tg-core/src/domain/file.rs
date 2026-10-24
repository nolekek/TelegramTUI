use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTransferStatus {
    Idle,
    Downloading { downloaded_size: i64 },
    Uploading { uploaded_size: i64 },
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: i32,
    pub size: i64,
    pub expected_size: i64,
    pub local_path: Option<PathBuf>,
    pub status: FileTransferStatus,
}

impl FileInfo {
    pub fn new(id: i32, size: i64, expected_size: i64) -> Self {
        Self {
            id,
            size,
            expected_size,
            local_path: None,
            status: FileTransferStatus::Idle,
        }
    }

    pub fn with_status(mut self, status: FileTransferStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_local_path(mut self, path: PathBuf) -> Self {
        self.local_path = Some(path);
        self
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, FileTransferStatus::Completed)
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self.status, FileTransferStatus::Downloading { .. })
    }

    pub fn is_uploading(&self) -> bool {
        matches!(self.status, FileTransferStatus::Uploading { .. })
    }

    pub fn can_be_downloaded(&self) -> bool {
        !self.is_completed() && !self.is_downloading()
    }

    pub fn downloaded_size(&self) -> i64 {
        match self.status {
            FileTransferStatus::Downloading { downloaded_size } => downloaded_size,
            FileTransferStatus::Completed => {
                if self.size > 0 {
                    self.size
                } else {
                    self.expected_size
                }
            }
            _ => 0,
        }
    }

    pub fn total_size(&self) -> i64 {
        if self.expected_size > 0 {
            self.expected_size
        } else if self.size > 0 {
            self.size
        } else {
            0
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.is_completed() {
            return 100.0;
        }
        let total = self.total_size();
        if total > 0 {
            let downloaded = self.downloaded_size();
            ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }
}
