//! Wallpaper and theme sources: a fixed image, or Caelestia's state files (followed live via inotify).

use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    None,
    File(PathBuf),
    Caelestia,
}

pub fn caelestia_dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    state.join("caelestia")
}

impl Source {
    pub fn parse(s: &str) -> Source {
        match s {
            "" | "none" => Source::None,
            "caelestia" => Source::Caelestia,
            p => Source::File(PathBuf::from(p)),
        }
    }

    /// The image path this source currently points to.
    pub fn resolve(&self) -> Option<PathBuf> {
        match self {
            Source::None => None,
            Source::File(p) => Some(p.clone()),
            Source::Caelestia => {
                let txt = std::fs::read_to_string(caelestia_dir().join("wallpaper/path.txt")).ok()?;
                let p = txt.trim();
                (!p.is_empty()).then(|| PathBuf::from(p))
            }
        }
    }
}

/// Decodes `path`, shrinking it (aspect kept) so it is no larger than needed to cover `cover` pixels.
pub fn load(path: &Path, cover: (u32, u32)) -> Result<image::RgbaImage, String> {
    let img = image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("{}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = (img.width(), img.height());
    let (cw, ch) = (cover.0.max(1) as f64, cover.1.max(1) as f64);
    // Cover-fit scale: the image must span the screen on both axes.
    let s = (cw / w as f64).max(ch / h as f64);
    let img = if s < 1.0 {
        let (nw, nh) = (((w as f64) * s).ceil() as u32, ((h as f64) * s).ceil() as u32);
        img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle)
    } else {
        img
    };
    Ok(img.into_rgba8())
}

/// Theme colours (primary, secondary, tertiary) from Caelestia's scheme.json, straight alpha.
pub fn caelestia_colours() -> Option<[[f32; 4]; 3]> {
    let txt = std::fs::read_to_string(caelestia_dir().join("scheme.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    let c = v.get("colours")?;
    let get = |k: &str| hex(c.get(k)?.as_str()?);
    Some([get("primary")?, get("secondary")?, get("tertiary")?])
}

pub const DEFAULT_COLOURS: [[f32; 4]; 3] = [[0.65, 0.78, 1.0, 1.0], [0.84, 0.75, 0.89, 1.0], [1.0, 0.71, 0.67, 1.0]];

fn hex(s: &str) -> Option<[f32; 4]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    let ch = |shift: u32| ((n >> shift) & 0xff) as f32 / 255.0;
    Some([ch(16), ch(8), ch(0), 1.0])
}

/// What changed under Caelestia's state directory.
#[derive(Debug, Clone, Copy)]
pub enum Change {
    Wallpaper,
    Theme,
}

/// Watches Caelestia's state dir with inotify (one blocked thread, no polling) and reports debounced changes.
pub fn watch(wallpaper: bool, theme: bool, tx: impl Fn(Change) + Send + 'static) {
    if !wallpaper && !theme {
        return;
    }
    watch_dir(caelestia_dir(), wallpaper, theme, tx);
}

/// Debounce window: changes arriving within it are reported together, once each.
const DEBOUNCE: Duration = Duration::from_millis(150);

fn watch_dir(dir: PathBuf, wallpaper: bool, theme: bool, tx: impl Fn(Change) + Send + 'static) {
    std::thread::Builder::new()
        .name("woofer-watch".into())
        .spawn(move || {
            use inotify::{Inotify, WatchMask};
            let Ok(mut ino) = Inotify::init() else { return };
            let mask = WatchMask::CLOSE_WRITE | WatchMask::MOVED_TO | WatchMask::CREATE;
            // Watch directories, not files: editors and Caelestia replace files atomically.
            let wd_theme = theme.then(|| ino.watches().add(&dir, mask).ok()).flatten();
            let wd_wall = wallpaper.then(|| ino.watches().add(dir.join("wallpaper"), mask).ok()).flatten();
            if wd_theme.is_none() && wd_wall.is_none() {
                log::info!("watch: {} not found, not following Caelestia", dir.display());
                return;
            }
            // (any event, wallpaper changed, theme changed)
            let scan = |events: inotify::Events<'_>| {
                let (mut any, mut w, mut t) = (false, false, false);
                for e in events {
                    any = true;
                    let name = e.name.and_then(|n| n.to_str()).unwrap_or("");
                    w |= Some(&e.wd) == wd_wall.as_ref() && name == "path.txt";
                    t |= Some(&e.wd) == wd_theme.as_ref() && name == "scheme.json";
                }
                (any, w, t)
            };
            let mut buf = [0u8; 4096];
            loop {
                let Ok(events) = ino.read_events_blocking(&mut buf) else { return };
                let (_, mut w, mut t) = scan(events);
                if !(w || t) {
                    continue;
                }
                // Debounce bursts of writes, but keep what they change: Caelestia writes scheme.json for the new
                // wallpaper right after path.txt, and dropping it would leave the old colours on screen.
                loop {
                    std::thread::sleep(DEBOUNCE);
                    // WouldBlock: nothing new since the last read.
                    let Ok(events) = ino.read_events(&mut buf) else { break };
                    let (any, w2, t2) = scan(events);
                    w |= w2;
                    t |= t2;
                    if !any {
                        break;
                    }
                }
                if w {
                    tx(Change::Wallpaper);
                }
                if t {
                    tx(Change::Theme);
                }
            }
        })
        .expect("spawn watch thread");
}

#[cfg(test)]
mod tests {
    #[test]
    fn hex_parses() {
        assert_eq!(super::hex("ff0080"), Some([1.0, 0.0, 128.0 / 255.0, 1.0]));
        assert_eq!(super::hex("nope"), None);
    }

    /// Caelestia's `set_wallpaper` writes path.txt, then scheme.json a few ms later: both must be reported.
    #[test]
    fn wallpaper_then_scheme_reports_both() {
        use super::{Change, DEBOUNCE, watch_dir};
        use std::sync::mpsc;
        let dir = std::env::temp_dir().join(format!("woofer-watch-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("wallpaper")).unwrap();
        let (tx, rx) = mpsc::channel();
        watch_dir(dir.clone(), true, true, move |c| {
            let _ = tx.send(c);
        });
        // Let the thread add its watches.
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::write(dir.join("wallpaper/path.txt"), "/tmp/wall.jpg").unwrap();
        std::thread::sleep(DEBOUNCE / 5);
        std::fs::write(dir.join("scheme.json"), "{}").unwrap();
        let wait = DEBOUNCE * 10;
        let got: Vec<Change> = [rx.recv_timeout(wait), rx.recv_timeout(wait)].into_iter().filter_map(Result::ok).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(got[..], [Change::Wallpaper, Change::Theme]), "{got:?}");
        assert!(rx.recv_timeout(DEBOUNCE * 2).is_err(), "each change is reported once");
    }
}
