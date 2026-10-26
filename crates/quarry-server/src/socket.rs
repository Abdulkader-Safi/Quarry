//! Where the per-user socket lives. Unix: a socket file inside a folder only this
//! user can open. Windows: a named pipe. `QUARRY_SOCKET` overrides both.

use interprocess::local_socket::{GenericFilePath, GenericNamespaced, Name, prelude::*};
use std::io;

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
    use std::os::unix::fs::DirBuilderExt;
    use std::path::PathBuf;
    // XDG_RUNTIME_DIR (Linux) and TMPDIR (macOS) are normally private to the user.
    let private = ["XDG_RUNTIME_DIR", "TMPDIR"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|dir| private_to_us(dir).is_ok());
    let dir = match private {
        Some(dir) => dir,
        None => {
            let dir = std::env::temp_dir().join(format!("quarry-{}", user()));
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            private_to_us(&dir)?;
            dir
        }
    };
    Ok(SocketAddr(
        dir.join("quarry.sock").to_string_lossy().into_owned(),
    ))
}

/// A real folder (not a symlink), owned by this user, closed to everyone else.
/// Anything else could let another local user sit on the socket path.
#[cfg(unix)]
fn private_to_us(dir: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::symlink_metadata(dir)?;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let us = unsafe { libc::geteuid() };
    if meta.is_dir() && meta.uid() == us && meta.permissions().mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{} is not a folder private to you; remove it or set QUARRY_SOCKET",
            dir.display()
        )))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn default_socket_sits_in_a_folder_only_we_own() {
        let addr = default_addr().unwrap();
        let dir = std::path::Path::new(&addr.0).parent().unwrap();
        private_to_us(dir).unwrap();
        // Shared /tmp itself is refused.
        assert!(private_to_us(std::path::Path::new("/tmp")).is_err());
    }
}
