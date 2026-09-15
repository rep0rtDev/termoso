//! Profile pictures: whatever the client uploads is decoded, centre-cropped
//! to a square, downscaled and re-encoded as a small lossy WebP before it
//! touches the database. Every stored avatar is therefore the same shape and
//! a few kilobytes, whatever the original was.
//!
//! Storage is behind [`store`], [`clear`] and [`load`]: today a Postgres
//! table, and nothing outside this module cares if that becomes an object
//! store. Clients only ever see `users.avatar_tag` and `GET /users/{id}/avatar`.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiResult, Error};

/// Largest upload the server bothers to decode. Clients downscale before
/// sending; this only guards against someone posting their whole camera roll.
pub const MAX_UPLOAD: usize = 4 * 1024 * 1024;
/// Stored side length: sharp on a 2× display at the 96 px the cabinet uses,
/// while the terminal clients draw it at 20–32 px.
pub const SIDE: u32 = 192;
/// Hard ceiling for a stored avatar; the quality ladder stops before this.
pub const MAX_STORED: usize = 48 * 1024;

const QUALITIES: [f32; 3] = [82.0, 70.0, 55.0];
const MAX_PIXELS: u64 = 40_000_000;

/// Decode, normalise and encode. Runs on the caller's thread; heavy for a
/// request handler, so call it from `spawn_blocking`.
pub fn normalize(input: &[u8]) -> ApiResult<Vec<u8>> {
    if input.len() > MAX_UPLOAD {
        return Err(Error::too_large("Image is too large (4 MiB max)"));
    }
    let img = decode(input)?;
    let square = img
        .resize_to_fill(SIDE, SIDE, FilterType::Lanczos3)
        .into_rgba8();
    let encoder = webp::Encoder::from_rgba(square.as_raw(), SIDE, SIDE);
    for q in QUALITIES {
        let out = encoder.encode(q);
        if out.len() <= MAX_STORED {
            return Ok(out.to_vec());
        }
    }
    Err(Error::bad_request("Could not compress this image"))
}

fn bad<E>(_: E) -> Error {
    Error::bad_request("Unsupported or corrupt image")
}

fn decode(input: &[u8]) -> ApiResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(input))
        .with_guessed_format()
        .map_err(bad)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(MAX_PIXELS * 4);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(bad)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(bad)?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Cache key for a stored avatar: short, content-derived, safe in a URL.
pub fn tag(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..16].to_string()
}

/// Replace the user's picture with an already normalised image; returns the
/// tag now advertised on the profile.
pub async fn store(db: &PgPool, user_id: Uuid, webp: &[u8]) -> ApiResult<String> {
    let tag = tag(webp);
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO user_avatars (user_id, tag, image) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET tag = EXCLUDED.tag, image = EXCLUDED.image, updated_at = now()",
    )
    .bind(user_id)
    .bind(&tag)
    .bind(webp)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET avatar_tag = $2, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .bind(&tag)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(tag)
}

pub async fn clear(db: &PgPool, user_id: Uuid) -> ApiResult<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM user_avatars WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET avatar_tag = NULL, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `(image, tag)` of a user's picture, if any.
pub async fn load(db: &PgPool, user_id: Uuid) -> ApiResult<Option<(Vec<u8>, String)>> {
    Ok(
        sqlx::query_as("SELECT image, tag FROM user_avatars WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| {
            Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        });
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn output_is_small_square_webp() {
        let out = normalize(&png(1600, 900)).unwrap();
        assert!(out.len() <= MAX_STORED, "{} bytes", out.len());
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (SIDE, SIDE));
    }

    #[test]
    fn tiny_input_is_upscaled_to_the_stored_side() {
        let img = image::load_from_memory(&normalize(&png(16, 24)).unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (SIDE, SIDE));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(normalize(b"definitely not an image").is_err());
        assert!(normalize(&vec![0u8; MAX_UPLOAD + 1]).is_err());
    }

    #[test]
    fn tag_depends_on_content() {
        assert_eq!(tag(b"a").len(), 16);
        assert_ne!(tag(b"a"), tag(b"b"));
    }
}
