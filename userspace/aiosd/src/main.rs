//! aiosd — local AI service daemon. Binds a Unix socket under the user runtime
//! dir and serves the request protocol. The key never leaves this process.
#![forbid(unsafe_code)]

use aiosd::{Request, Response, State};
use libcredentials::SystemdCredsStore;
use libipc::{PeerCred, Server};
use std::path::PathBuf;
use std::sync::Mutex;

fn socket_path() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("ai-native-os/aiosd.sock")
}

fn cred_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
                .join(".local/share")
        });
    base.join("ai-native-os/credentials")
}

fn main() -> std::io::Result<()> {
    let store = SystemdCredsStore::new(cred_dir()).expect("credential store");
    let state = Mutex::new(State::new(store));
    let path = socket_path();
    let server = Server::bind(&path)?;
    eprintln!("aiosd listening on {}", path.display());
    server.serve(move |_cred: PeerCred, req: Request| -> Response {
        state.lock().unwrap().dispatch(req)
    })
}
