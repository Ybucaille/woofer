//! `reactive-desktop ctl …`: one line per connection over a unix socket in $XDG_RUNTIME_DIR.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    dir.join("reactive-desktop.sock")
}

#[derive(Debug)]
pub enum Command {
    Preset(String),
    Intensity(f32),
    Pause,
    Resume,
    Status,
}

impl Command {
    pub fn parse(line: &str) -> Result<Command, String> {
        let mut it = line.split_whitespace();
        let cmd = it.next().unwrap_or("");
        let arg = it.next();
        Ok(match (cmd, arg) {
            ("preset", Some(id)) => Command::Preset(id.to_owned()),
            ("intensity", Some(x)) => Command::Intensity(
                x.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0).ok_or("intensity: expected a number ≥ 0")?,
            ),
            ("pause", None) => Command::Pause,
            ("resume", None) => Command::Resume,
            ("status", None) => Command::Status,
            _ => return Err(format!("unknown command {line:?}; expected preset <id> | intensity <x> | pause | resume | status")),
        })
    }
}

pub type Request = (Command, mpsc::Sender<String>);

/// Binds the socket (refusing if another daemon answers on it) and serves it from a thread.
pub fn serve(tx: impl Fn(Request) + Send + 'static) -> Result<(), String> {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return Err(format!("another instance is running ({})", path.display()));
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::thread::Builder::new()
        .name("rd-ctl".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                let mut r = BufReader::new(&conn);
                if r.read_line(&mut line).is_err() {
                    continue;
                }
                let reply = match Command::parse(line.trim()) {
                    Ok(cmd) => {
                        let (rtx, rrx) = mpsc::channel();
                        tx((cmd, rtx));
                        rrx.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| "error: daemon busy".into())
                    }
                    Err(e) => format!("error: {e}"),
                };
                let _ = (&conn).write_all(reply.as_bytes());
                let _ = (&conn).write_all(b"\n");
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_socket() {
    let _ = std::fs::remove_file(socket_path());
}

/// Client side. Returns the process exit code.
pub fn client(args: &[String]) -> i32 {
    let line = args.join(" ");
    if let Err(e) = Command::parse(&line) {
        eprintln!("{e}");
        return 2;
    }
    let path = socket_path();
    let mut s = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot reach the daemon at {}: {e}", path.display());
            return 1;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    if s.write_all(format!("{line}\n").as_bytes()).is_err() {
        return 1;
    }
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    let out = out.trim_end();
    if let Some(e) = out.strip_prefix("error: ") {
        eprintln!("{e}");
        return 1;
    }
    if !out.is_empty() {
        println!("{out}");
    }
    0
}
