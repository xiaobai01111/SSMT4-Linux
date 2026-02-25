use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

/// Download a file with resume support using .temp intermediate file.
/// Mirrors the logic from LutheringLaves.py `download_file_with_resume`.
pub async fn download_with_resume(
    client: &Client,
    url: &str,
    file_path: &Path,
    overwrite: bool,
    progress_callback: Option<&(dyn Fn(u64) + Send + Sync)>,
    cancel_token: Option<Arc<AsyncMutex<bool>>>,
) -> Result<(), String> {
    if let Some(token) = &cancel_token {
        if *token.lock().await {
            return Err("Download cancelled".to_string());
        }
    }

    // If file exists and not overwriting, skip
    if file_path.exists() && !overwrite {
        if let Some(cb) = progress_callback {
            if let Ok(meta) = tokio::fs::metadata(file_path).await {
                cb(meta.len());
            }
        }
        return Ok(());
    }

    // If overwriting, remove existing file
    if file_path.exists() && overwrite {
        tokio::fs::remove_file(file_path)
            .await
            .map_err(|e| format!("Failed to remove existing file: {}", e))?;
    }

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let temp_path = file_path.with_extension(
        format!(
            "{}.temp",
            file_path.extension().unwrap_or_default().to_string_lossy()
        )
        .trim_start_matches('.'),
    );

    // Check how much we already have in temp file
    let mut downloaded_bytes: u64 = 0;
    if temp_path.exists() {
        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
            downloaded_bytes = meta.len();
        }
    }

    // Build request with Range header for resume
    let mut req = client.get(url).header("User-Agent", "Mozilla/5.0");

    if downloaded_bytes > 0 {
        req = req.header("Range", format!("bytes={}-", downloaded_bytes));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    let status = resp.status().as_u16();

    match status {
        206 => {
            // Partial content - resume supported
        }
        200 => {
            // Full content - server doesn't support resume or fresh download
            if downloaded_bytes > 0 {
                warn!("Server doesn't support resume for {}, restarting", url);
                downloaded_bytes = 0;
            }
        }
        416 => {
            // Range not satisfiable - file might be complete
            if temp_path.exists() {
                tokio::fs::rename(&temp_path, file_path)
                    .await
                    .map_err(|e| format!("Failed to rename temp file: {}", e))?;
                return Ok(());
            }
            return Err(format!("HTTP 416 and no temp file exists for {}", url));
        }
        _ => {
            return Err(format!("Unexpected HTTP status {} for {}", status, url));
        }
    }

    // Open file for writing (append if resuming)
    let mut file = if downloaded_bytes > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&temp_path)
            .await
            .map_err(|e| format!("Failed to open temp file for append: {}", e))?
    } else {
        tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| format!("Failed to create temp file: {}", e))?
    };

    // Stream download in 1MB chunks
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        if let Some(token) = &cancel_token {
            if *token.lock().await {
                file.flush()
                    .await
                    .map_err(|e| format!("Flush error before cancel: {}", e))?;
                return Err("Download cancelled".to_string());
            }
        }

        let chunk = chunk_result.map_err(|e| format!("Stream error: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        let _n = chunk.len() as u64;
        downloaded_bytes += _n;
        let _ = downloaded_bytes; // 保留变量用于断点续传计算

        if let Some(cb) = progress_callback {
            cb(chunk.len() as u64);
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("Flush error: {}", e))?;
    drop(file);

    // Rename temp file to final path
    tokio::fs::rename(&temp_path, file_path)
        .await
        .map_err(|e| format!("Failed to rename temp to final: {}", e))?;

    Ok(())
}

/// Simple non-resumable download for small files (tools, etc.)
pub async fn download_simple(client: &Client, url: &str, dest: &Path) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    info!("Downloading {} -> {}", url, dest.display());

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let resp = client
        .get(url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Failed to create file: {}", e))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed to read response: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush file: {}", e))?;

    Ok(())
}
