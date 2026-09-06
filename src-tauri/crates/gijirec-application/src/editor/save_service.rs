//! Persists transcript session snapshots under a JST-dated subdirectory tree.

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Asia::Tokyo;
use gijirec_domain::editor::{
    AiTranscriptionJsonlRecord, EditorError, SaveFileFailure, SaveTranscriptSessionRequest,
    SaveTranscriptSessionResult,
};
use std::io;
use std::path::{Path, PathBuf};

const HANDWRITING_FILE: &str = "handwriting.md";
const AI_TRANSCRIPTION_FILE: &str = "ai-transcription.md";
const AI_TRANSCRIPTION_JSONL_FILE: &str = "ai-transcription.jsonl";

type WriteFn = Box<dyn Fn(&Path, &[u8]) -> io::Result<()>>;

/// Writes Markdown / JSONL session exports under an injected save directory.
pub struct SaveService {
    save_directory: Option<PathBuf>,
    write_fn: Option<WriteFn>,
}

impl SaveService {
    pub fn new(save_directory: Option<PathBuf>) -> Self {
        Self {
            save_directory,
            write_fn: None,
        }
    }

    #[cfg(test)]
    fn with_write_fn(save_directory: Option<PathBuf>, write_fn: WriteFn) -> Self {
        Self {
            save_directory,
            write_fn: Some(write_fn),
        }
    }

    pub fn save(
        &self,
        request: &SaveTranscriptSessionRequest,
    ) -> Result<SaveTranscriptSessionResult, EditorError> {
        self.save_at(request, Utc::now())
    }

    pub fn save_at(
        &self,
        request: &SaveTranscriptSessionRequest,
        now: DateTime<Utc>,
    ) -> Result<SaveTranscriptSessionResult, EditorError> {
        let base_dir = self.resolve_base_directory()?;
        let output_dir = prepare_session_output_dir(&base_dir, now)?;
        let files_to_write = build_session_file_payloads(request)?;
        let (files_written, files_failed) =
            self.write_session_files(&base_dir, &output_dir, &files_to_write)?;
        Ok(assemble_save_result(
            output_dir,
            files_written,
            files_failed,
        ))
    }

    fn resolve_base_directory(&self) -> Result<PathBuf, EditorError> {
        let configured = self
            .save_directory
            .as_ref()
            .ok_or(EditorError::SaveDirectoryNotSet)?;
        canonicalize_save_directory(configured)
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if let Some(write_fn) = &self.write_fn {
            write_fn(path, contents)
        } else {
            std::fs::write(path, contents)
        }
    }

    fn write_session_files(
        &self,
        base_dir: &Path,
        output_dir: &Path,
        files_to_write: &[(&str, Vec<u8>)],
    ) -> Result<(Vec<String>, Vec<SaveFileFailure>), EditorError> {
        let mut files_written = Vec::new();
        let mut files_failed = Vec::new();

        for (file_name, contents) in files_to_write {
            let file_path = output_dir.join(file_name);
            assert_path_under_base(base_dir, &file_path)?;
            match self.write_bytes(&file_path, contents) {
                Ok(()) => files_written.push(file_path.to_string_lossy().into_owned()),
                Err(err) => {
                    files_failed.push(SaveFileFailure {
                        path: file_path.to_string_lossy().into_owned(),
                        reason_ja: format!("書き込みに失敗しました。（{err}）"),
                    });
                }
            }
        }

        Ok((files_written, files_failed))
    }
}

fn prepare_session_output_dir(base_dir: &Path, now: DateTime<Utc>) -> Result<PathBuf, EditorError> {
    let jst = now.with_timezone(&Tokyo);
    let day_dir = base_dir
        .join(format!("{:04}", jst.year()))
        .join(format!("{:02}", jst.month()))
        .join(format!("{:02}", jst.day()));
    let base_name = format!("{:02}_{:02}_{:02}", jst.hour(), jst.minute(), jst.second());
    let session_name = resolve_session_dir_name(&day_dir, &base_name);
    let output_dir = day_dir.join(&session_name);

    assert_path_under_base(base_dir, &output_dir)?;

    if let Some(parent) = output_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|err| EditorError::SaveDirectoryCreateFailed {
            detail: format!("create day directory: {err}"),
        })?;
    }
    std::fs::create_dir_all(&output_dir).map_err(|err| EditorError::SaveDirectoryCreateFailed {
        detail: format!("create session directory: {err}"),
    })?;

    Ok(output_dir)
}

fn build_session_file_payloads(
    request: &SaveTranscriptSessionRequest,
) -> Result<Vec<(&'static str, Vec<u8>)>, EditorError> {
    let mut files_to_write: Vec<(&str, Vec<u8>)> = vec![
        (
            HANDWRITING_FILE,
            request.handwriting_markdown.as_bytes().to_vec(),
        ),
        (
            AI_TRANSCRIPTION_FILE,
            request.ai_transcription_markdown.as_bytes().to_vec(),
        ),
    ];
    if let Some(records) = &request.ai_transcription_jsonl {
        files_to_write.push((AI_TRANSCRIPTION_JSONL_FILE, serialize_jsonl(records)?));
    }
    Ok(files_to_write)
}

fn assemble_save_result(
    output_dir: PathBuf,
    files_written: Vec<String>,
    files_failed: Vec<SaveFileFailure>,
) -> SaveTranscriptSessionResult {
    let output_directory = output_dir.to_string_lossy().into_owned();

    if files_failed.is_empty() {
        return SaveTranscriptSessionResult {
            success: true,
            output_directory: Some(output_directory),
            files_written: Some(files_written),
            files_failed: None,
            error: None,
        };
    }

    let failed_count = files_failed.len();
    let written_count = files_written.len();
    SaveTranscriptSessionResult {
        success: false,
        output_directory: Some(output_directory),
        files_written: if files_written.is_empty() {
            None
        } else {
            Some(files_written)
        },
        files_failed: Some(files_failed),
        error: Some(
            EditorError::SavePartialFailure {
                detail: format!(
                    "{} of {} files failed",
                    failed_count,
                    failed_count + written_count
                ),
            }
            .to_user_facing(),
        ),
    }
}

fn canonicalize_save_directory(path: &Path) -> Result<PathBuf, EditorError> {
    if !path.exists() {
        return Err(EditorError::SaveDirectoryUnavailable {
            detail: format!("save directory does not exist: {}", path.display()),
        });
    }
    let canonical = path
        .canonicalize()
        .map_err(|err| EditorError::SaveDirectoryUnavailable {
            detail: format!("canonicalize save directory: {err}"),
        })?;
    if !canonical.is_dir() {
        return Err(EditorError::SaveDirectoryUnavailable {
            detail: format!("save directory is not a directory: {}", canonical.display()),
        });
    }
    Ok(canonical)
}

fn assert_path_under_base(base: &Path, candidate: &Path) -> Result<(), EditorError> {
    let base = base
        .canonicalize()
        .map_err(|err| EditorError::SaveDirectoryUnavailable {
            detail: format!("canonicalize base directory: {err}"),
        })?;

    if candidate
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(EditorError::SaveDirectoryUnavailable {
            detail: "path traversal via parent directory component".to_string(),
        });
    }

    if candidate.strip_prefix(&base).is_ok() {
        return Ok(());
    }

    if candidate.exists() {
        let canonical =
            candidate
                .canonicalize()
                .map_err(|err| EditorError::SaveDirectoryUnavailable {
                    detail: format!("canonicalize candidate path: {err}"),
                })?;
        if canonical.starts_with(&base) {
            return Ok(());
        }
    }

    Err(EditorError::SaveDirectoryUnavailable {
        detail: format!("path escapes save directory: {}", candidate.display()),
    })
}

fn resolve_session_dir_name(day_dir: &Path, base_name: &str) -> String {
    if !day_dir.join(base_name).exists() {
        return base_name.to_string();
    }
    let mut suffix = 1u32;
    while day_dir.join(format!("{base_name}_{suffix:03}")).exists() {
        suffix += 1;
    }
    format!("{base_name}_{suffix:03}")
}

fn serialize_jsonl(records: &[AiTranscriptionJsonlRecord]) -> Result<Vec<u8>, EditorError> {
    let mut buffer = Vec::new();
    for record in records {
        let line = serde_json::to_string(record).map_err(|err| EditorError::Internal {
            detail: format!("serialize jsonl record: {err}"),
        })?;
        buffer.extend_from_slice(line.as_bytes());
        buffer.push(b'\n');
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::{
        AI_TRANSCRIPTION_FILE, AI_TRANSCRIPTION_JSONL_FILE, HANDWRITING_FILE, SaveService,
        assert_path_under_base,
    };
    use chrono::{TimeZone, Utc};
    use gijirec_domain::editor::{
        AiTranscriptionJsonlRecord, EditorError, EditorUserErrorCode, SaveTranscriptSessionRequest,
    };
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use uuid::Uuid;

    fn temp_save_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gijirec-save-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp save root");
        dir
    }

    fn sample_request() -> SaveTranscriptSessionRequest {
        SaveTranscriptSessionRequest {
            session_id: "session-test".to_string(),
            handwriting_markdown: "# notes".to_string(),
            ai_transcription_markdown: "hello world".to_string(),
            ai_transcription_jsonl: None,
        }
    }

    fn request_with_jsonl() -> SaveTranscriptSessionRequest {
        SaveTranscriptSessionRequest {
            session_id: "session-jsonl".to_string(),
            handwriting_markdown: "# notes".to_string(),
            ai_transcription_markdown: "hello".to_string(),
            ai_transcription_jsonl: Some(vec![AiTranscriptionJsonlRecord {
                block_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
                sequence: 1,
                text: "hello".to_string(),
                start_timestamp_ms: 1_500,
                language: "ja".to_string(),
            }]),
        }
    }

    fn fixed_jst_time() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 5, 15, 30, 45)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn save_directory_not_set_returns_error() {
        let service = SaveService::new(None);
        let err = service
            .save_at(&sample_request(), fixed_jst_time())
            .expect_err("save must fail");

        assert!(matches!(err, EditorError::SaveDirectoryNotSet));
        assert_eq!(
            err.to_user_facing().code,
            EditorUserErrorCode::SaveDirectoryNotSet
        );
    }

    #[test]
    fn jst_subdirectory_path_format() {
        let root = temp_save_root();
        let service = SaveService::new(Some(root.clone()));

        let result = service
            .save_at(&sample_request(), fixed_jst_time())
            .expect("save succeeds");

        assert!(result.success);
        let output = result.output_directory.expect("output directory");
        let normalized = output.replace('\\', "/");
        assert!(
            normalized.ends_with("2026/09/06/00_30_45"),
            "expected JST path suffix, got {normalized}"
        );

        assert!(Path::new(&output).join(HANDWRITING_FILE).is_file());
        assert!(Path::new(&output).join(AI_TRANSCRIPTION_FILE).is_file());
        assert!(
            !Path::new(&output)
                .join(AI_TRANSCRIPTION_JSONL_FILE)
                .exists()
        );
    }

    #[test]
    fn same_second_collision_uses_001_suffix() {
        let root = temp_save_root();
        let service = SaveService::new(Some(root.clone()));
        let time = fixed_jst_time();

        let first = service
            .save_at(&sample_request(), time)
            .expect("first save");
        let second = service
            .save_at(&sample_request(), time)
            .expect("second save");

        assert!(first.success);
        assert!(second.success);

        let first_dir = first.output_directory.expect("first output");
        let second_dir = second.output_directory.expect("second output");
        assert_ne!(first_dir, second_dir);

        let second_normalized = second_dir.replace('\\', "/");
        assert!(
            second_normalized.ends_with("2026/09/06/00_30_45_001"),
            "expected _001 collision suffix, got {second_normalized}"
        );
    }

    #[test]
    fn rejects_path_outside_canonical_base() {
        let root = temp_save_root().canonicalize().expect("canonicalize root");
        let outside = root
            .parent()
            .expect("parent")
            .join(format!("gijirec-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&outside).expect("create outside dir");
        let outside = outside.canonicalize().expect("canonicalize outside");

        let err = assert_path_under_base(&root, &outside).expect_err("must reject escape");
        assert!(matches!(err, EditorError::SaveDirectoryUnavailable { .. }));
        assert_eq!(
            err.to_user_facing().code,
            EditorUserErrorCode::SaveDirectoryUnavailable
        );
    }

    #[test]
    fn rejects_save_directory_that_is_not_a_directory() {
        let root = temp_save_root();
        let file_path = root.join("not_a_dir.txt");
        fs::write(&file_path, "x").expect("create blocking file");

        let service = SaveService::new(Some(file_path));
        let err = service
            .save_at(&sample_request(), fixed_jst_time())
            .expect_err("save must fail");

        assert!(matches!(err, EditorError::SaveDirectoryUnavailable { .. }));
    }

    #[test]
    fn partial_failure_keeps_successful_files() {
        let root = temp_save_root();
        let fail_target = Arc::new(Mutex::new(AI_TRANSCRIPTION_FILE.to_string()));
        let fail_name = fail_target.clone();

        let service = SaveService::with_write_fn(
            Some(root.clone()),
            Box::new(move |path, contents| {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if file_name == *fail_name.lock().expect("lock fail target") {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked"));
                }
                std::fs::write(path, contents)
            }),
        );

        let result = service
            .save_at(&sample_request(), fixed_jst_time())
            .expect("partial save returns Ok");

        assert!(!result.success);
        let written = result.files_written.expect("files written");
        assert_eq!(written.len(), 1);
        assert!(written[0].replace('\\', "/").ends_with(HANDWRITING_FILE));

        let failed = result.files_failed.expect("files failed");
        assert_eq!(failed.len(), 1);
        assert!(
            failed[0]
                .path
                .replace('\\', "/")
                .ends_with(AI_TRANSCRIPTION_FILE)
        );
        assert!(!failed[0].reason_ja.is_empty());

        let error = result.error.expect("user error");
        assert_eq!(error.code, EditorUserErrorCode::SavePartialFailure);

        let output = result.output_directory.expect("output directory");
        assert!(Path::new(&output).join(HANDWRITING_FILE).is_file());
        assert!(!Path::new(&output).join(AI_TRANSCRIPTION_FILE).exists());
    }

    #[test]
    fn writes_jsonl_when_payload_present() {
        let root = temp_save_root();
        let service = SaveService::new(Some(root.clone()));

        let result = service
            .save_at(&request_with_jsonl(), fixed_jst_time())
            .expect("save with jsonl");

        assert!(result.success);
        let output = result.output_directory.expect("output directory");
        let jsonl_path = Path::new(&output).join(AI_TRANSCRIPTION_JSONL_FILE);
        assert!(jsonl_path.is_file());
        let contents = fs::read_to_string(jsonl_path).expect("read jsonl");
        assert!(contents.contains("\"block_id\""));
        assert!(contents.contains("\"sequence\""));
    }

    #[test]
    fn writes_markdown_files_with_request_content() {
        let root = temp_save_root();
        let service = SaveService::new(Some(root.clone()));
        let request = sample_request();

        let result = service
            .save_at(&request, fixed_jst_time())
            .expect("save markdown files");

        assert!(result.success);
        let output = result.output_directory.expect("output directory");
        let handwriting = fs::read_to_string(Path::new(&output).join(HANDWRITING_FILE))
            .expect("read handwriting markdown");
        let ai = fs::read_to_string(Path::new(&output).join(AI_TRANSCRIPTION_FILE))
            .expect("read ai transcription markdown");

        assert_eq!(handwriting, request.handwriting_markdown);
        assert_eq!(ai, request.ai_transcription_markdown);
    }

    #[test]
    fn save_100kb_payload_completes_under_500ms() {
        const TARGET_BYTES: usize = 100 * 1024;
        const MAX_ELAPSED_MS: u128 = 500;

        let handwriting = "H".repeat(TARGET_BYTES / 2);
        let ai = "A".repeat(TARGET_BYTES - handwriting.len());
        let total_bytes = handwriting.len() + ai.len();
        assert_eq!(total_bytes, TARGET_BYTES);

        let root = temp_save_root();
        let service = SaveService::new(Some(root.clone()));
        let request = SaveTranscriptSessionRequest {
            session_id: "perf-100kb".to_string(),
            handwriting_markdown: handwriting,
            ai_transcription_markdown: ai,
            ai_transcription_jsonl: None,
        };

        let started = Instant::now();
        let result = service
            .save_at(&request, fixed_jst_time())
            .expect("save 100kb payload");
        let elapsed_ms = started.elapsed().as_millis();

        assert!(result.success);
        eprintln!(
            "save_100kb_perf: bytes={} elapsed_ms={} threshold_ms={}",
            total_bytes, elapsed_ms, MAX_ELAPSED_MS
        );
        assert!(
            elapsed_ms < MAX_ELAPSED_MS,
            "save took {elapsed_ms} ms, expected < {MAX_ELAPSED_MS} ms"
        );
    }
}
