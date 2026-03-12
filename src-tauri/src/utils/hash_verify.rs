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
        let actual = sha256_file(path).await?.trim().to_ascii_lowercase();
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
        let actual = md5_file(path).await?.trim().to_ascii_lowercase();
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

#[cfg(test)]
mod tests {
    use super::{md5_file, sha256_file, verify_file_integrity, VerifiedHashAlgo};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join("ssmt4-tests")
            .join(format!("hash-verify-{label}-{nonce}.txt"))
    }

    #[tokio::test]
    async fn sha256_and_md5_file_match_known_hello_digest() {
        let path = unique_temp_path("digests");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create temp dir");
        }
        std::fs::write(&path, b"hello").expect("write test file");

        let sha256 = sha256_file(&path).await.expect("hash sha256");
        let md5 = md5_file(&path).await.expect("hash md5");

        assert_eq!(
            sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(md5, "5d41402abc4b2a76b9719d911017c592");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn verify_file_integrity_prefers_sha256_and_accepts_trimmed_expected_values() {
        let path = unique_temp_path("verify-ok");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create temp dir");
        }
        std::fs::write(&path, b"hello").expect("write test file");

        let algo = verify_file_integrity(
            &path,
            5,
            Some(" 2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824 "),
            Some("5d41402abc4b2a76b9719d911017c592"),
        )
        .await
        .expect("verify file");

        assert_eq!(algo, VerifiedHashAlgo::Sha256);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn verify_file_integrity_reports_size_and_checksum_failures() {
        let path = unique_temp_path("verify-fail");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create temp dir");
        }
        std::fs::write(&path, b"hello").expect("write test file");

        let size_err =
            verify_file_integrity(&path, 4, None, Some("5d41402abc4b2a76b9719d911017c592"))
                .await
                .expect_err("size mismatch should fail");
        assert!(size_err.contains("Size mismatch"));

        let md5_err =
            verify_file_integrity(&path, 5, None, Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))
                .await
                .expect_err("md5 mismatch should fail");
        assert!(md5_err.contains("MD5 mismatch"));

        let missing_err = verify_file_integrity(&path, 5, None, None)
            .await
            .expect_err("missing checksum should fail");
        assert!(missing_err.contains("Missing checksum metadata"));

        let _ = std::fs::remove_file(path);
    }
}
