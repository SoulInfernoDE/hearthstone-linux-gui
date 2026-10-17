//! The launcher logo: a looping animation of Tux at the tavern counter.
//!
//! The animation plays through GTK's media backend (GStreamer). Until it is
//! ready, and whenever it cannot be played, the first frame is shown as a
//! still picture instead. It pauses while the window is minimised or the
//! game runs, so it costs nothing when nobody looks at it.

use super::components::embedded_image;
use relm4::gtk::{self, gdk, glib, prelude::*};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

const ANIMATION: &[u8] = include_bytes!("../../data/logo/logo-loop.webm");
const STILL: &[u8] = include_bytes!("../../data/logo/logo-still.png");
/// Logo size in the window; both files have twice this size for HiDPI.
const SIZE: i32 = 184;

pub struct Logo {
    pub image: gtk::Image,
    /// Holds the media file for the life of the window; without this
    /// reference it would be dropped before it starts playing.
    animation: Option<Rc<Animation>>,
}

struct Animation {
    media: gtk::MediaFile,
    minimized: Cell<bool>,
    game_running: Cell<bool>,
}

impl Animation {
    fn apply(&self) {
        if self.minimized.get() || self.game_running.get() {
            self.media.pause();
        } else {
            self.media.play();
        }
    }
}

impl Logo {
    pub fn new(cache_dir: &Path) -> Self {
        let image = embedded_image(STILL);
        image.set_pixel_size(SIZE);
        let animation = match write_animation(cache_dir) {
            Ok(path) => Some(animate(&image, &path)),
            Err(error) => {
                tracing::warn!(%error, "logo animation unavailable; showing the still logo");
                None
            }
        };
        Self { image, animation }
    }

    pub fn set_game_running(&self, running: bool) {
        if let Some(animation) = &self.animation {
            if animation.game_running.replace(running) != running {
                animation.apply();
            }
        }
    }
}

fn animate(image: &gtk::Image, path: &Path) -> Rc<Animation> {
    let still = image.paintable();
    let media = gtk::MediaFile::for_filename(path);
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
    let animation = Rc::new(Animation {
        media,
        minimized: Cell::new(false),
        game_running: Cell::new(false),
    });

    // Follow the window's minimised state once the window exists.
    let watching = Cell::new(false);
    image.connect_map(glib::clone!(
        #[weak]
        animation,
        move |image| {
            if watching.replace(true) {
                return;
            }
            let Some(toplevel) = image
                .native()
                .and_then(|native| native.surface())
                .and_downcast::<gdk::Toplevel>()
            else {
                return;
            };
            toplevel.connect_state_notify(glib::clone!(
                #[weak]
                animation,
                move |toplevel| {
                    let minimized = toplevel.state().contains(gdk::ToplevelState::MINIMIZED);
                    if animation.minimized.replace(minimized) != minimized {
                        animation.apply();
                    }
                }
            ));
        }
    ));

    animation.apply();
    animation
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
