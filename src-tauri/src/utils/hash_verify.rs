use md5::{Digest as _, Md5};
use sha2::Sha256;
use std::path::Path;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedHashAlgo {
    Sha256,
    Md5,
}

pub async fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("Failed to open file {}: {}", path.display(), e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub async fn md5_file(path: &Path) -> Result<String, String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("Failed to open file {}: {}", path.display(), e))?;
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[allow(dead_code)]
pub async fn verify_sha256(path: &Path, expected: &str) -> Result<bool, String> {
    let actual = sha256_file(path).await?;
    Ok(actual == expected)
}

fn normalize_expected(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
}

pub async fn verify_file_integrity(
    path: &Path,
    expected_size: u64,
    expected_sha256: Option<&str>,
    expected_md5: Option<&str>,
) -> Result<VerifiedHashAlgo, String> {
    if expected_size > 0 {
        let actual_size = tokio::fs::metadata(path)
            .await
            .map_err(|e| format!("Failed to read metadata {}: {}", path.display(), e))?
            .len();
        if actual_size != expected_size {
            return Err(format!(
                "Size mismatch for {} (expected: {}, got: {})",
                path.display(),
                expected_size,
                actual_size
            ));
        }
    }

    if let Some(expected) = normalize_expected(expected_sha256) {
        let actual = sha256_file(path)
            .await?
            .trim()
            .to_ascii_lowercase();
        if actual != expected {
            return Err(format!(
                "SHA256 mismatch for {} (expected: {}, got: {})",
                path.display(),
                expected,
                actual
            ));
        }
        return Ok(VerifiedHashAlgo::Sha256);
    }

    if let Some(expected) = normalize_expected(expected_md5) {
        let actual = md5_file(path)
            .await?
            .trim()
            .to_ascii_lowercase();
        if actual != expected {
            return Err(format!(
                "MD5 mismatch for {} (expected: {}, got: {})",
                path.display(),
                expected,
                actual
            ));
        }
        return Ok(VerifiedHashAlgo::Md5);
    }

    Err(format!(
        "Missing checksum metadata for {} (need SHA256 or MD5)",
        path.display()
    ))
}
