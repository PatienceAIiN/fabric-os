//! Minimal authenticated IPC over Unix domain sockets.
//!
//! Requests/responses are line-delimited JSON. The server learns the caller's
//! kernel-verified uid/pid/gid via SO_PEERCRED (never a self-asserted name).
//! Transport is deliberately tiny and dependency-light; CBOR/protobuf can
//! replace JSON at hot paths later (ADR-0004) without changing the shape.
//!
//! `unsafe` is confined to the single SO_PEERCRED getsockopt call, documented
//! below.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerCred {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
}

/// Read the peer credentials of a connected Unix socket (SO_PEERCRED).
pub fn peer_cred(stream: &UnixStream) -> std::io::Result<PeerCred> {
    use std::os::unix::io::AsRawFd;
    #[repr(C)]
    struct Ucred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    let fd = stream.as_raw_fd();
    let mut cred = Ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<Ucred>() as u32;
    // SAFETY: getsockopt writes at most `len` bytes into `cred`, which is a
    // correctly sized #[repr(C)] struct; fd is a valid connected socket owned
    // by `stream`. SOL_SOCKET=1, SO_PEERCRED=17 on Linux.
    let rc = unsafe {
        libc_getsockopt(
            fd,
            1,  // SOL_SOCKET
            17, // SO_PEERCRED
            &mut cred as *mut Ucred as *mut core::ffi::c_void,
            &mut len as *mut u32,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(PeerCred {
        pid: cred.pid,
        uid: cred.uid,
        gid: cred.gid,
    })
}

extern "C" {
    #[link_name = "getsockopt"]
    fn libc_getsockopt(
        fd: i32,
        level: i32,
        optname: i32,
        optval: *mut core::ffi::c_void,
        optlen: *mut u32,
    ) -> i32;
}

/// A JSON line-protocol server bound to a Unix socket path.
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
}

impl Server {
    /// Bind, replacing any stale socket file. Socket is created 0600.
    pub fn bind(path: impl AsRef<Path>) -> std::io::Result<Server> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Server { listener, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Serve requests. `handler(cred, request) -> response` is called per line.
    /// One thread per connection. Blocks forever (until process exit).
    pub fn serve<Req, Res, H>(&self, handler: H) -> std::io::Result<()>
    where
        Req: DeserializeOwned,
        Res: Serialize,
        H: Fn(PeerCred, Req) -> Res + Send + Sync + 'static,
    {
        let handler = std::sync::Arc::new(handler);
        for conn in self.listener.incoming() {
            let stream = match conn {
                Ok(s) => s,
                Err(_) => continue,
            };
            let h = handler.clone();
            std::thread::spawn(move || {
                let _ = handle_conn::<Req, Res, _>(stream, h);
            });
        }
        Ok(())
    }

    /// Serve exactly `n` connections then return (used by tests).
    pub fn serve_n<Req, Res, H>(&self, n: usize, handler: H) -> std::io::Result<()>
    where
        Req: DeserializeOwned,
        Res: Serialize,
        H: Fn(PeerCred, Req) -> Res,
    {
        let mut count = 0;
        for conn in self.listener.incoming() {
            let stream = conn?;
            let cred = peer_cred(&stream).unwrap_or(PeerCred {
                pid: 0,
                uid: 0,
                gid: 0,
            });
            let mut reader = BufReader::new(stream.try_clone()?);
            let mut stream = stream;
            let mut line = String::new();
            if reader.read_line(&mut line)? > 0 {
                if let Ok(req) = serde_json::from_str::<Req>(line.trim()) {
                    let res = handler(cred, req);
                    let mut out = serde_json::to_string(&res).unwrap_or_default();
                    out.push('\n');
                    stream.write_all(out.as_bytes())?;
                }
            }
            count += 1;
            if count >= n {
                break;
            }
        }
        Ok(())
    }
}

fn handle_conn<Req, Res, H>(stream: UnixStream, handler: std::sync::Arc<H>) -> std::io::Result<()>
where
    Req: DeserializeOwned,
    Res: Serialize,
    H: Fn(PeerCred, Req) -> Res,
{
    let cred = peer_cred(&stream).unwrap_or(PeerCred {
        pid: 0,
        uid: 0,
        gid: 0,
    });
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Req>(&line) {
            Ok(req) => {
                let res = handler(cred, req);
                let mut out = serde_json::to_string(&res).unwrap_or_default();
                out.push('\n');
                writer.write_all(out.as_bytes())?;
                writer.flush()?;
            }
            Err(_) => break,
        }
    }
    Ok(())
}

/// Send one request and read one response (client side).
pub fn request<Req: Serialize, Res: DeserializeOwned>(
    path: impl AsRef<Path>,
    req: &Req,
) -> std::io::Result<Res> {
    let mut stream = UnixStream::connect(path)?;
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp)?;
    serde_json::from_str(resp.trim())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize)]
    struct Req {
        add: (i64, i64),
    }
    #[derive(Serialize, Deserialize)]
    struct Res {
        sum: i64,
        caller_uid: u32,
    }

    #[test]
    fn round_trip_with_peer_cred() {
        let path = std::env::temp_dir().join(format!("ipc-test-{}.sock", std::process::id()));
        let server = Server::bind(&path).unwrap();
        let sp = path.clone();
        let h = std::thread::spawn(move || {
            server
                .serve_n(1, |cred: PeerCred, r: Req| Res {
                    sum: r.add.0 + r.add.1,
                    caller_uid: cred.uid,
                })
                .unwrap();
        });
        let res: Res = request(&sp, &Req { add: (2, 3) }).unwrap();
        assert_eq!(res.sum, 5);
        // peer uid is our own uid (kernel-verified, not asserted)
        assert_eq!(res.caller_uid, unsafe { geteuid() });
        h.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    extern "C" {
        fn geteuid() -> u32;
    }
}
