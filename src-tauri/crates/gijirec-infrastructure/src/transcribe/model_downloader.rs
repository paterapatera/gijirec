//! HTTPS model download with streaming progress and atomic file placement.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use gijirec_domain::transcribe::TranscribeError;
use reqwest::StatusCode;
use reqwest::blocking::Client;

/// Progress payload per `docs/contracts/whisper-transcribe-status.md`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelDownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub percent: Option<f64>,
    pub status: ModelDownloadStatus,
}

/// Download lifecycle status emitted through progress callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDownloadStatus {
    Downloading,
    Verifying,
    Complete,
    Failed,
}

/// Downloads whisper model files over HTTPS (TLS 1.2+) with resumable-safe cleanup.
pub struct ModelDownloader {
    client: Client,
}

impl ModelDownloader {
    pub fn new() -> Result<Self, TranscribeError> {
        let client = Client::builder().https_only(true).build().map_err(|err| {
            TranscribeError::Internal {
                detail: format!("failed to build HTTPS client: {err}"),
            }
        })?;
        Ok(Self { client })
    }

    /// Builds a downloader that also accepts plain HTTP (for local mock servers in tests).
    #[cfg(test)]
    fn new_allow_http() -> Result<Self, TranscribeError> {
        let client = Client::builder()
            .build()
            .map_err(|err| TranscribeError::Internal {
                detail: format!("failed to build HTTP client: {err}"),
            })?;
        Ok(Self { client })
    }

    #[allow(clippy::too_many_lines)]
    pub fn download<F>(
        &self,
        url: &str,
        destination: &Path,
        mut on_progress: F,
    ) -> Result<(), TranscribeError>
    where
        F: FnMut(ModelDownloadProgress),
    {
        let part_path = partial_path(destination);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                download_failed(
                    DownloadFailureContext {
                        part_path: &part_path,
                        destination,
                        downloaded: 0,
                        total: None,
                        on_progress: &mut on_progress,
                    },
                    err,
                )
            })?;
        }

        let response = self.client.get(url).send().map_err(|err| {
            download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded: 0,
                    total: None,
                    on_progress: &mut on_progress,
                },
                err,
            )
        })?;

        if response.status() != StatusCode::OK {
            return Err(download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded: 0,
                    total: None,
                    on_progress: &mut on_progress,
                },
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unexpected HTTP status {}", response.status()),
                ),
            ));
        }

        let bytes_total = response.content_length();
        let mut downloaded: u64 = 0;
        emit_progress(
            &mut on_progress,
            downloaded,
            bytes_total,
            ModelDownloadStatus::Downloading,
        );

        let mut part_file = File::create(&part_path).map_err(|err| {
            download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded,
                    total: bytes_total,
                    on_progress: &mut on_progress,
                },
                err,
            )
        })?;
        let mut reader = response;
        let mut buffer = [0u8; 8192];

        loop {
            let read = match reader.read(&mut buffer) {
                Ok(r) => r,
                Err(err) => {
                    return Err(download_failed(
                        DownloadFailureContext {
                            part_path: &part_path,
                            destination,
                            downloaded,
                            total: bytes_total,
                            on_progress: &mut on_progress,
                        },
                        err,
                    ));
                }
            };
            if read == 0 {
                break;
            }

            if let Err(err) = part_file.write_all(&buffer[..read]) {
                return Err(download_failed(
                    DownloadFailureContext {
                        part_path: &part_path,
                        destination,
                        downloaded,
                        total: bytes_total,
                        on_progress: &mut on_progress,
                    },
                    err,
                ));
            }
            downloaded += read as u64;
            emit_progress(
                &mut on_progress,
                downloaded,
                bytes_total,
                ModelDownloadStatus::Downloading,
            );
        }

        if let Some(total) = bytes_total
            && downloaded < total
        {
            return Err(download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded,
                    total: bytes_total,
                    on_progress: &mut on_progress,
                },
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("connection closed after {downloaded} of {total} bytes"),
                ),
            ));
        }

        part_file.sync_all().map_err(|err| {
            download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded,
                    total: bytes_total,
                    on_progress: &mut on_progress,
                },
                err,
            )
        })?;

        finalize_download(&part_path, destination).map_err(|err| {
            download_failed(
                DownloadFailureContext {
                    part_path: &part_path,
                    destination,
                    downloaded,
                    total: bytes_total,
                    on_progress: &mut on_progress,
                },
                err,
            )
        })?;

        emit_progress(
            &mut on_progress,
            downloaded,
            bytes_total,
            ModelDownloadStatus::Complete,
        );
        Ok(())
    }
}

fn partial_path(destination: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", destination.display()))
}

fn emit_progress<F>(
    on_progress: &mut F,
    downloaded: u64,
    total: Option<u64>,
    status: ModelDownloadStatus,
) where
    F: FnMut(ModelDownloadProgress),
{
    on_progress(ModelDownloadProgress {
        bytes_downloaded: downloaded,
        bytes_total: total,
        percent: percent(downloaded, total),
        status,
    });
}

fn percent(downloaded: u64, total: Option<u64>) -> Option<f64> {
    total.map(|total_bytes| {
        if total_bytes == 0 {
            100.0
        } else {
            (downloaded as f64 / total_bytes as f64) * 100.0
        }
    })
}

fn finalize_download(part_path: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(part_path, destination)?;
    Ok(())
}

fn cleanup_partial_files(part_path: &Path, destination: &Path) {
    let _ = fs::remove_file(part_path);
    if destination.exists() {
        let _ = fs::remove_file(destination);
    }
}

struct DownloadFailureContext<'a, F> {
    part_path: &'a Path,
    destination: &'a Path,
    downloaded: u64,
    total: Option<u64>,
    on_progress: &'a mut F,
}

fn download_failed<F>(
    ctx: DownloadFailureContext<'_, F>,
    err: impl std::fmt::Display,
) -> TranscribeError
where
    F: FnMut(ModelDownloadProgress),
{
    cleanup_partial_files(ctx.part_path, ctx.destination);
    emit_progress(
        ctx.on_progress,
        ctx.downloaded,
        ctx.total,
        ModelDownloadStatus::Failed,
    );
    TranscribeError::ModelDownloadFailed {
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::sync::{Arc, Barrier};
    use std::thread::{self, JoinHandle};

    use gijirec_domain::transcribe::TranscribeErrorCode;

    use super::*;

    struct MockHttpServer {
        base_url: String,
        handle: JoinHandle<()>,
    }

    impl MockHttpServer {
        fn spawn<F>(handler: F) -> Self
        where
            F: Fn(&mut TcpStream) + Send + Sync + 'static,
        {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
            let addr = listener.local_addr().expect("mock server addr");
            let handler = Arc::new(handler);
            let accept_barrier = Arc::new(Barrier::new(2));
            let accept_barrier_for_thread = Arc::clone(&accept_barrier);

            let handle = thread::spawn(move || {
                accept_barrier_for_thread.wait();
                let (mut stream, _) = listener.accept().expect("accept mock request");
                drain_http_request(&mut stream);
                handler(&mut stream);
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Write);
            });

            accept_barrier.wait();

            Self {
                base_url: format!("http://{addr}"),
                handle,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base_url, path)
        }

        fn shutdown(self) {
            self.handle.join().expect("join mock server");
        }
    }

    fn drain_http_request(stream: &mut TcpStream) {
        let mut buffer = [0u8; 1024];
        let mut request = Vec::new();
        while request.windows(4).all(|window| window != b"\r\n\r\n") {
            let read = stream.read(&mut buffer).expect("read mock request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.len() > 16 * 1024 {
                break;
            }
        }
    }

    fn temp_destination(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "gijirec-model-downloader-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        (base.join("model.bin"), base)
    }

    fn cleanup_temp(base: &Path) {
        let _ = fs::remove_dir_all(base);
    }

    fn write_http_response(stream: &mut TcpStream, status: u16, body: &[u8]) {
        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write headers");
        stream.write_all(body).expect("write body");
    }

    #[test]
    fn successful_download_writes_destination_and_emits_progress() {
        let body = b"mock-whisper-model-bytes";
        let server = MockHttpServer::spawn(move |stream| {
            write_http_response(stream, 200, body);
        });

        let (destination, base) = temp_destination("success");
        let part_path = partial_path(&destination);
        let mut progress_updates = Vec::new();

        let downloader = ModelDownloader::new_allow_http().expect("downloader");
        downloader
            .download(&server.url("/model.bin"), &destination, |progress| {
                progress_updates.push(progress)
            })
            .expect("download should succeed");

        assert_eq!(fs::read(&destination).expect("read destination"), body);
        assert!(
            !part_path.exists(),
            ".part file must not remain after success"
        );
        assert!(
            progress_updates
                .iter()
                .any(|p| p.status == ModelDownloadStatus::Downloading),
            "must emit downloading progress"
        );
        let complete = progress_updates.last().expect("progress updates present");
        assert_eq!(complete.status, ModelDownloadStatus::Complete);
        assert_eq!(complete.bytes_downloaded, body.len() as u64);
        assert_eq!(complete.bytes_total, Some(body.len() as u64));
        assert_eq!(complete.percent, Some(100.0));

        server.shutdown();
        cleanup_temp(&base);
    }

    #[test]
    fn http_404_removes_part_file_and_returns_model_download_failed() {
        let server = MockHttpServer::spawn(|stream| {
            let response =
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).expect("write 404");
        });

        let (destination, base) = temp_destination("404");
        let part_path = partial_path(&destination);
        let downloader = ModelDownloader::new_allow_http().expect("downloader");

        let err = downloader
            .download(&server.url("/missing"), &destination, |_| {})
            .expect_err("404 must fail");

        assert!(matches!(err, TranscribeError::ModelDownloadFailed { .. }));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelDownloadFailed
        );
        assert!(!part_path.exists(), ".part file must be removed on failure");
        assert!(
            !destination.exists(),
            "destination must not remain on failure"
        );

        server.shutdown();
        cleanup_temp(&base);
    }

    #[test]
    fn failed_download_emits_failed_progress_status() {
        let server = MockHttpServer::spawn(|stream| {
            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).expect("write 500");
        });

        let (destination, base) = temp_destination("failed-status");
        let mut last_status = None;
        let downloader = ModelDownloader::new_allow_http().expect("downloader");

        let _ = downloader
            .download(&server.url("/model.bin"), &destination, |progress| {
                last_status = Some(progress.status)
            })
            .expect_err("server error must fail");

        assert_eq!(last_status, Some(ModelDownloadStatus::Failed));

        server.shutdown();
        cleanup_temp(&base);
    }

    #[test]
    fn interrupted_download_removes_part_file_and_returns_model_download_failed() {
        let body = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let server = MockHttpServer::spawn(move |stream| {
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(headers.as_bytes()).expect("write headers");
            stream
                .write_all(&body[..8])
                .expect("write partial body before interruption");
            stream.shutdown(Shutdown::Write).expect("shutdown write");
        });

        let (destination, base) = temp_destination("interrupt");
        let part_path = partial_path(&destination);
        let downloader = ModelDownloader::new_allow_http().expect("downloader");

        let err = downloader
            .download(&server.url("/model.bin"), &destination, |_| {})
            .expect_err("interrupted download must fail");

        assert!(matches!(err, TranscribeError::ModelDownloadFailed { .. }));
        assert!(
            !part_path.exists(),
            ".part file must be removed on interruption"
        );
        assert!(
            !destination.exists(),
            "destination must not remain on interruption"
        );

        server.shutdown();
        cleanup_temp(&base);
    }

    #[test]
    fn partial_path_suffix_is_part() {
        let destination = Path::new("/tmp/models/ggml-small.bin");
        assert_eq!(
            partial_path(destination),
            PathBuf::from("/tmp/models/ggml-small.bin.part")
        );
    }

    #[test]
    fn percent_is_none_when_total_unknown() {
        assert_eq!(percent(10, None), None);
    }
}
