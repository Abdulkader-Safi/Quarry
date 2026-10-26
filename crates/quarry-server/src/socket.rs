//! Where the per-user socket lives. Unix: a socket file inside a folder only this
//! user can open. Windows: a named pipe. `QUARRY_SOCKET` overrides both.

use interprocess::local_socket::{GenericFilePath, GenericNamespaced, Name, prelude::*};
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct SocketAddr(pub String);

impl SocketAddr {
    /// `QUARRY_SOCKET`, else the per-user default.
    pub fn from_env() -> io::Result<Self> {
        if let Some(path) = std::env::var_os("QUARRY_SOCKET") {
            return Ok(Self(path.to_string_lossy().into_owned()));
        }
        default_addr()
    }

    pub fn name(&self) -> io::Result<Name<'_>> {
        if cfg!(windows) {
            self.0.as_str().to_ns_name::<GenericNamespaced>()
        } else {
            self.0.as_str().to_fs_name::<GenericFilePath>()
        }
    }
}

fn user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into())
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

#[cfg(windows)]
fn default_addr() -> io::Result<SocketAddr> {
    // ponytail: the pipe uses Windows' default security (owner and admins full,
    // Everyone read). Add an owner-only DACL if a shared machine needs it.
    Ok(SocketAddr(format!("quarry-{}", user())))
}

#[cfg(unix)]
fn default_addr() -> io::Result<SocketAddr> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    // XDG_RUNTIME_DIR (Linux) and TMPDIR (macOS) are already private to the user.
    let private = ["XDG_RUNTIME_DIR", "TMPDIR"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|dir| {
            std::fs::metadata(dir).is_ok_and(|m| m.is_dir() && m.permissions().mode() & 0o077 == 0)
        });
    let dir = match private {
        Some(dir) => dir,
        None => {
            let dir = std::env::temp_dir().join(format!("quarry-{}", user()));
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            let mode = std::fs::metadata(&dir)?.permissions().mode();
            if mode & 0o077 != 0 {
                return Err(io::Error::other(format!(
                    "{} is readable by other users; remove it or set QUARRY_SOCKET",
                    dir.display()
                )));
            }
            dir
        }
    };
    Ok(SocketAddr(
        dir.join("quarry.sock").to_string_lossy().into_owned(),
    ))
}
