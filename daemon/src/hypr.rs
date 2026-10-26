//! Hyprland integration: which monitors currently show a fullscreen window. Absent (no thread) elsewhere.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

/// Monitor name → its visible workspace has a fullscreen window.
pub type Fullscreen = HashMap<String, bool>;

fn socket_dir() -> Option<PathBuf> {
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    let new = runtime.join("hypr").join(&sig);
    if new.is_dir() {
        return Some(new);
    }
    Some(PathBuf::from("/tmp/hypr").join(sig))
}

fn query(dir: &std::path::Path, cmd: &str) -> Option<serde_json::Value> {
    let mut s = UnixStream::connect(dir.join(".socket.sock")).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    s.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    serde_json::from_str(&out).ok()
}

fn snapshot(dir: &std::path::Path) -> Option<Fullscreen> {
    fullscreen_map(&query(dir, "j/monitors")?, &query(dir, "j/workspaces")?)
}

/// A monitor counts as fullscreen when its active (or open special) workspace has a fullscreen window.
fn fullscreen_map(monitors: &serde_json::Value, workspaces: &serde_json::Value) -> Option<Fullscreen> {
    let full = |id: i64| {
        workspaces.as_array().into_iter().flatten().any(|w| {
            w["id"].as_i64() == Some(id) && w["hasfullscreen"].as_bool() == Some(true)
        })
    };
    let mut map = Fullscreen::new();
    for m in monitors.as_array()? {
        let Some(name) = m["name"].as_str() else { continue };
        let active = m["activeWorkspace"]["id"].as_i64().is_some_and(full);
        let special = m["specialWorkspace"]["id"].as_i64().is_some_and(|id| id != 0 && full(id));
        map.insert(name.to_owned(), active || special);
    }
    Some(map)
}

const EVENTS: &[&str] = &[
    "fullscreen", "workspace", "workspacev2", "focusedmon", "focusedmonv2", "moveworkspace", "moveworkspacev2",
    "activespecial", "activespecialv2", "openwindow", "closewindow", "movewindow", "movewindowv2",
    "monitoradded", "monitoraddedv2", "monitorremoved", "monitorremovedv2", "changefloatingmode",
];

/// Starts the listener if running under Hyprland. `tx` gets a fresh snapshot after every relevant event.
pub fn spawn(tx: impl Fn(Fullscreen) + Send + 'static) -> bool {
    let Some(dir) = socket_dir() else { return false };
    std::thread::Builder::new()
        .name("rd-hypr".into())
        .spawn(move || {
            loop {
                if let Some(s) = snapshot(&dir) {
                    tx(s);
                }
                let Ok(ev) = UnixStream::connect(dir.join(".socket2.sock")) else {
                    log::warn!("hyprland: event socket unavailable, fullscreen pause disabled");
                    return;
                };
                let mut lines = BufReader::new(ev);
                let mut line = String::new();
                loop {
                    line.clear();
                    match lines.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    let name = line.split(">>").next().unwrap_or("");
                    if EVENTS.contains(&name)
                        && let Some(s) = snapshot(&dir) {
                            tx(s);
                        }
                }
                // Socket closed: reconnect after a pause; the connect above ends the thread if Hyprland is gone.
                std::thread::sleep(Duration::from_secs(2));
            }
        })
        .expect("spawn hyprland thread");
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_fullscreen_per_monitor() {
        let m = serde_json::json!([
            {"name": "eDP-1", "activeWorkspace": {"id": 2}, "specialWorkspace": {"id": 0}},
            {"name": "HDMI-A-1", "activeWorkspace": {"id": 5}, "specialWorkspace": {"id": -98}},
            {"name": "DP-2", "activeWorkspace": {"id": 7}, "specialWorkspace": {"id": 0}}
        ]);
        let w = serde_json::json!([
            {"id": 2, "hasfullscreen": true}, {"id": 5, "hasfullscreen": false},
            {"id": -98, "hasfullscreen": true}, {"id": 7, "hasfullscreen": false}
        ]);
        let f = super::fullscreen_map(&m, &w).unwrap();
        assert_eq!(f["eDP-1"], true);
        assert_eq!(f["HDMI-A-1"], true, "special workspace on top");
        assert_eq!(f["DP-2"], false);
    }
}
