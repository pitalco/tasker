use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum file size in bytes (50 MB)
pub const MAX_FILE_SIZE: i64 = 50 * 1024 * 1024;

/// A file stored in the database, associated with a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFile {
    pub id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    #[serde(skip)] // Don't serialize blob content by default
    pub content: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RunFile {
    /// Create a new RunFile with auto-detected MIME type
    pub fn new(
        run_id: String,
        workflow_id: Option<String>,
        file_path: String,
        content: Vec<u8>,
    ) -> Self {
        let file_name = file_path
            .rsplit('/')
            .next()
            .unwrap_or(&file_path)
            .to_string();

        let mime_type = mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string();

        let file_size = content.len() as i64;
        let now = Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            run_id,
            workflow_id,
            file_name,
            file_path,
            mime_type,
            file_size,
            content,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if the file size is within the allowed limit
    pub fn is_size_valid(&self) -> bool {
        self.file_size <= MAX_FILE_SIZE
    }
}

/// File metadata without content (for listing files)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFileMetadata {
    pub id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Denormalized run name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_name: Option<String>,
    /// Denormalized workflow name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
}

impl From<&RunFile> for RunFileMetadata {
    fn from(file: &RunFile) -> Self {
        Self {
            id: file.id.clone(),
            run_id: file.run_id.clone(),
            workflow_id: file.workflow_id.clone(),
            file_name: file.file_name.clone(),
            file_path: file.file_path.clone(),
            mime_type: file.mime_type.clone(),
            file_size: file.file_size,
            created_at: file.created_at,
            updated_at: file.updated_at,
            run_name: None,
            workflow_name: None,
        }
    }
}

/// Response for file content (with base64 encoding for API transport)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFileContent {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub content_base64: String,
}

impl From<RunFile> for RunFileContent {
    fn from(file: RunFile) -> Self {
        use base64::Engine;
        Self {
            id: file.id,
            file_name: file.file_name,
            file_path: file.file_path,
            mime_type: file.mime_type,
            file_size: file.file_size,
            content_base64: base64::engine::general_purpose::STANDARD.encode(&file.content),
        }
    }
}

/// Response for listing files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    pub files: Vec<RunFileMetadata>,
    pub total: i64,
}
