//! The launcher logo: a looping animation of Tux at the tavern counter.
//!
//! The animation plays through GTK's media backend (GStreamer). Until it is
//! ready, and whenever it cannot be played, the first frame is shown as a
//! still picture instead.

use super::components::embedded_image;
use relm4::gtk::{self, glib, prelude::*};
use std::path::{Path, PathBuf};

const ANIMATION: &[u8] = include_bytes!("../../data/logo/logo-loop.webm");
const STILL: &[u8] = include_bytes!("../../data/logo/logo-still.png");
/// Logo size in the window; both files have twice this size for HiDPI.
const SIZE: i32 = 184;

pub fn widget(cache_dir: &Path) -> gtk::Image {
    let image = embedded_image(STILL);
    image.set_pixel_size(SIZE);
    let still = image.paintable();

    let path = match write_animation(cache_dir) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "logo animation unavailable; showing the still logo");
            return image;
        }
    };
    let media = gtk::MediaFile::for_filename(&path);
    media.set_loop(true);
    media.set_muted(true);
    media.connect_prepared_notify(glib::clone!(
        #[weak]
        image,
        move |media| {
            if media.is_prepared() && media.error().is_none() {
                image.set_paintable(Some(media));
            }
        }
    ));
    // The handler keeps the media file alive for the life of the window.
    media.connect_error_notify(glib::clone!(
        #[weak]
        image,
        move |media| {
            if let Some(error) = media.error() {
                tracing::warn!(%error, "logo animation cannot be played; showing the still logo");
                image.set_paintable(still.as_ref());
            }
        }
    ));
    media.play();
    image
}

/// GTK's media backend plays files, not memory, so the embedded animation is
/// written to the cache directory once per version of the file.
fn write_animation(cache_dir: &Path) -> std::io::Result<PathBuf> {
    let path = cache_dir.join(format!("logo-loop-{:x}.webm", fingerprint(ANIMATION)));
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() == ANIMATION.len() as u64) {
        return Ok(path);
    }
    std::fs::create_dir_all(cache_dir)?;
    let partial = path.with_extension("webm.part");
    std::fs::write(&partial, ANIMATION)?;
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

/// FNV-1a; only tells versions of the embedded file apart.
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
