use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
};
use tauri::{Emitter, Manager};

pub enum Claim {
    Primary {
        listener: UnixListener,
        path: PathBuf,
    },
    Existing,
}

fn socket_path() -> Result<PathBuf, String> {
    let runtime = if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime)
    } else {
        let uid = fs::metadata("/proc/self")
            .map(|metadata| metadata.uid())
            .unwrap_or(0);
        let runtime = std::env::temp_dir().join(format!("flow-linux-{uid}"));
        fs::create_dir_all(&runtime)
            .map_err(|e| format!("Could not create instance directory: {e}"))?;
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Could not protect instance directory: {e}"))?;
        runtime
    };
    Ok(runtime.join("flow-linux.sock"))
}

pub fn claim(show_existing: bool) -> Result<Claim, String> {
    let path = socket_path()?;
    if let Ok(mut stream) = UnixStream::connect(&path) {
        if show_existing {
            stream
                .write_all(b"s")
                .map_err(|e| format!("Could not open the running Flow window: {e}"))?;
        }
        return Ok(Claim::Existing);
    }
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Could not clear an old Flow instance socket: {e}"))?;
    }
    let listener = UnixListener::bind(&path)
        .map_err(|e| format!("Could not start Flow's per-user instance socket: {e}"))?;
    if let Err(error) = fs::set_permissions(&path, fs::Permissions::from_mode(0o600)) {
        let _ = fs::remove_file(&path);
        return Err(format!("Could not protect Flow's instance socket: {error}"));
    }
    Ok(Claim::Primary { listener, path })
}

pub fn serve(app: tauri::AppHandle, listener: UnixListener, path: PathBuf) {
    std::thread::spawn(move || {
        let _cleanup = SocketCleanup(path);
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else {
                break;
            };
            let mut request = [0; 1];
            if stream.read_exact(&mut request).is_ok() && request[0] == b's' {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("flow-status-refresh", ());
                }
            }
        }
    });
}

struct SocketCleanup(PathBuf);
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
