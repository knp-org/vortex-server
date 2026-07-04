
//! Stateless image helpers. Kept as free functions (no DB pool / shared state),
//! so this is not a `*Service` struct like the data-access services.
//!
//! Two responsibilities:
//! - [`download_image`]: cache a remote metadata image (posters/covers) on disk.
//! - [`read_exif`] / [`ImageExif`]: extract photo metadata from a local image file
//!   at scan time for the Images (photo gallery) library type.

/// Supported photo file extensions for Images libraries.
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "heic", "heif"];

/// Photo metadata extracted from an image file (EXIF + pixel dimensions).
/// Every field is optional: images may carry no EXIF at all.
#[derive(Debug, Default, Clone)]
pub struct ImageExif {
    /// Capture time, EXIF `DateTimeOriginal` ("YYYY:MM:DD HH:MM:SS").
    pub taken_at: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<i64>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub orientation: Option<i64>,
}

/// Read pixel dimensions and EXIF tags from an image file.
///
/// Blocking (does synchronous file IO); call via `tokio::task::spawn_blocking`.
/// Never fails: anything unreadable simply yields `None` for that field.
pub fn read_exif(path: &str) -> ImageExif {
    let mut out = ImageExif::default();

    // Pixel dimensions come from the image header directly (works even without EXIF).
    if let Ok(size) = imagesize::size(path) {
        out.width = Some(size.width as i64);
        out.height = Some(size.height as i64);
    }

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return out,
    };
    let mut reader = std::io::BufReader::new(&file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        // No EXIF block (common for PNG/screenshots) — dimensions are still returned.
        Err(_) => return out,
    };

    use exif::{In, Tag};

    let str_field = |tag: Tag| {
        exif.get_field(tag, In::PRIMARY).map(|f| {
            f.display_value().to_string().trim_matches('"').trim().to_string()
        }).filter(|s| !s.is_empty())
    };

    out.taken_at = str_field(Tag::DateTimeOriginal).or_else(|| str_field(Tag::DateTime));
    out.camera_make = str_field(Tag::Make);
    out.camera_model = str_field(Tag::Model);
    out.lens = str_field(Tag::LensModel);

    out.iso = exif.get_field(Tag::PhotographicSensitivity, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .map(|v| v as i64);

    out.focal_length = exif.get_field(Tag::FocalLength, In::PRIMARY)
        .and_then(|f| rational_f64(&f.value));
    out.aperture = exif.get_field(Tag::FNumber, In::PRIMARY)
        .and_then(|f| rational_f64(&f.value));

    out.orientation = exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .map(|v| v as i64);

    out.gps_lat = gps_coord(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, 'S');
    out.gps_lon = gps_coord(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef, 'W');

    out
}

/// First rational component of an EXIF value as `f64`.
fn rational_f64(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(v) => v.first().map(|r| r.to_f64()),
        _ => None,
    }
}

/// Decode a GPS coordinate (degrees/minutes/seconds triple + hemisphere ref) to
/// signed decimal degrees. `neg_ref` is the ref letter ('S' or 'W') that negates.
fn gps_coord(exif: &exif::Exif, coord: exif::Tag, reference: exif::Tag, neg_ref: char) -> Option<f64> {
    use exif::{In, Value};
    let field = exif.get_field(coord, In::PRIMARY)?;
    let dms = match &field.value {
        Value::Rational(v) if v.len() >= 3 => v,
        _ => return None,
    };
    let degrees = dms[0].to_f64() + dms[1].to_f64() / 60.0 + dms[2].to_f64() / 3600.0;
    let negate = exif.get_field(reference, In::PRIMARY)
        .map(|f| f.display_value().to_string().to_uppercase().contains(neg_ref))
        .unwrap_or(false);
    Some(if negate { -degrees } else { degrees })
}

/// Download an image from a URL and save it locally
/// Returns the filename of the saved image
pub async fn download_image(url: &str) -> std::result::Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    if url.is_empty() {
        return Ok(None);
    }

    // Ensure thumbnails directory exists
    let cfg = crate::infrastructure::config::config();
    let images_dir = cfg.data_dir.join("thumbnails");
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
