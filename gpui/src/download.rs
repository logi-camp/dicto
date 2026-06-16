use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub current_file: String,
}

pub type SharedProgress = Arc<Mutex<DownloadProgress>>;

pub fn new_shared_progress() -> SharedProgress {
    Arc::new(Mutex::new(DownloadProgress {
        bytes_downloaded: 0,
        total_bytes: 0,
        speed_bytes_per_sec: 0,
        current_file: String::new(),
    }))
}

fn download_file(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    progress: &SharedProgress,
) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = PathBuf::from(format!("{}.part", dest.display()));

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    let mut reader = resp;
    let mut file = std::fs::File::create(&tmp_path)?;
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    let start = Instant::now();
    let mut last_report = Instant::now();
    let mut buf = vec![0u8; 64 * 1024];

    let filename = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        hasher.update(&buf[..n]);
        downloaded += n as u64;

        if last_report.elapsed().as_millis() >= 100 {
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64 / elapsed) as u64
            } else {
                0
            };
            if let Ok(mut p) = progress.lock() {
                p.bytes_downloaded = downloaded;
                p.total_bytes = total_bytes;
                p.speed_bytes_per_sec = speed;
                p.current_file = filename.clone();
            }
            last_report = Instant::now();
        }
    }

    drop(file);

    if !expected_sha256.is_empty() {
        let actual = format!("{:x}", hasher.finalize());
        if actual != expected_sha256 {
            let _ = std::fs::remove_file(&tmp_path);
            anyhow::bail!("SHA256 mismatch: expected {expected_sha256}, got {actual}");
        }
    }

    std::fs::rename(&tmp_path, dest)?;

    Ok(())
}

pub fn download_entry(
    urls: &[(String, PathBuf)],
    sha256s: &[String],
    progress: &SharedProgress,
) -> anyhow::Result<()> {
    for (i, (url, dest)) in urls.iter().enumerate() {
        let expected = sha256s.get(i).map(|s| s.as_str()).unwrap_or("");
        download_file(url, dest, expected, progress)?;
    }
    Ok(())
}
