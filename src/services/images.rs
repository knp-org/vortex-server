use std::path::PathBuf;


/// Download an image from a URL and save it locally
/// Returns the filename of the saved image
pub async fn download_image(url: &str) -> std::result::Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    if url.is_empty() {
        return Ok(None);
    }

    // Ensure thumbnails directory exists
    let images_dir = PathBuf::from("thumbnails");
    if !images_dir.exists() {
        tokio::fs::create_dir_all(&images_dir).await?;
    }

    // Generate filename based on hash of URL to avoid duplicates and handle weird chars
    let digest = md5::compute(url.as_bytes());
    let hash = format!("{:x}", digest);
    
    // Determine extension
    let ext = if url.to_lowercase().ends_with(".png") {
        "png"
    } else {
        "jpg"
    };
    
    let filename = format!("{}.{}", hash, ext);
    let file_path = images_dir.join(&filename);

    // If already exists, just return it
    if file_path.exists() {
        return Ok(Some(filename));
    }

    // Download
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response.bytes().await?;
    tokio::fs::write(&file_path, &bytes).await?;

    Ok(Some(filename))
}
