//! `aios` — ai-native-os control tool.
//!
//! A light, dependency-free CLI over the deterministic core. It is a functional
//! stand-in for the AI command bar / settings until the graphical shell (M3+).
#![forbid(unsafe_code)]

use aios::{claude_status, run_demo1, run_demo3, set_claude_key, CLAUDE_KEY_NAME};
use libcredentials::{CredentialStore, SystemdCredsStore};
use std::path::PathBuf;

fn data_base() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
                .join(".local/share")
        })
}
fn cred_dir() -> PathBuf {
    data_base().join("ai-native-os/credentials")
}
fn settings_path() -> PathBuf {
    let cfg = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())).join(".config")
        });
    cfg.join("ai-native-os/settings.json")
}

fn usage() -> ! {
    eprintln!(
        "aios — ai-native-os control tool\n\
         \n\
         USAGE:\n\
         \x20 aios demo1                 Intent + security + rollback demonstration\n\
         \x20 aios demo3                 Multi-agent delegation + revocation\n\
         \x20 aios settings status       Show AI provider status (key masked)\n\
         \x20 aios settings set-key KEY  Store the Claude API key securely\n\
         \x20 aios settings remove-key   Remove the stored Claude API key\n\
         \x20 aios settings list         Show all settings\n\
         \x20 aios settings get KEY      Show one setting\n\
         \x20 aios settings set KEY VAL  Change a setting (validated)\n\
         \x20 aios ask TEXT...           AI command bar (routes local/cloud)\n\
         \x20 aios ask-private TEXT...   Force local (never send to cloud)\n\
         \x20 aios activity              AI activity center (agents/intents/blocked)\n\
         \x20 aios monitor               System monitor (resources + processes)\n"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["demo1"] => {
            let sandbox = std::env::temp_dir().join("aios-demo1");
            let r = run_demo1(&sandbox).expect("demo1 runs");
            println!("Intent #organize — proposed & executed inside a transaction");
            println!("  Files seen:        {}", r.files_seen);
            println!("  Files moved:       {}", r.files_moved);
            println!("  Files deleted:     {}", r.files_deleted);
            println!("  Network requests:  {}", r.network_requests);
            println!("  External uploads:  0");
            println!();
            if r.injection_blocked {
                println!("  Injected document requested: net.connect:evil.com:443");
                println!("  BLOCKED — reason: {}", r.block_reason);
            }
            println!();
            println!(
                "  Rollback restored files: {}",
                if r.rolled_back_ok { "yes" } else { "NO" }
            );
            println!(
                "  Provenance chain valid:  {} ({} events)",
                r.provenance_ok, r.provenance_events
            );
        }
        ["demo3"] => {
            let r = run_demo3();
            println!("Multi-agent: ResearchAgent -> (delegates read) -> DataAgent");
            println!(
                "  DataAgent read delegated cap:      {}",
                r.data_agent_read_before
            );
            println!(
                "  DataAgent denied parent's network: {}",
                r.data_agent_net_denied
            );
            println!("  Revoke ResearchAgent...");
            println!(
                "  DataAgent read after revoke:       {}",
                r.data_agent_read_after_revoke
            );
        }
        ["settings", "status"] => {
            let store = SystemdCredsStore::new(cred_dir()).expect("cred store");
            println!("{}", claude_status(&store));
        }
        ["settings", "set-key", key] => {
            let mut store = SystemdCredsStore::new(cred_dir()).expect("cred store");
            match set_claude_key(&mut store, key) {
                Ok(()) => {
                    println!("✓ Claude key stored securely (encrypted at rest, never displayed).")
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        ["settings", "remove-key"] => {
            let mut store = SystemdCredsStore::new(cred_dir()).expect("cred store");
            store.delete(CLAUDE_KEY_NAME).ok();
            println!("✓ Claude key removed.");
        }
        ["settings", "list"] => {
            let s = libsettings::Settings::load(&settings_path());
            for k in libsettings::Settings::keys() {
                println!("  {k:<26} = {}", s.get(k).unwrap_or_default());
            }
        }
        ["settings", "get", key] => {
            let s = libsettings::Settings::load(&settings_path());
            match s.get(key) {
                Some(v) => println!("{v}"),
                None => {
                    eprintln!("unknown key: {key}");
                    std::process::exit(1);
                }
            }
        }
        ["settings", "set", key, val] => {
            let path = settings_path();
            let mut s = libsettings::Settings::load(&path);
            match s.set(key, val) {
                Ok(()) => {
                    s.save(&path).expect("save settings");
                    println!("✓ {key} = {val}");
                }
                Err(e) => {
                    eprintln!("error: {}", e.0);
                    std::process::exit(1);
                }
            }
        }
        ["ask", rest @ ..] if !rest.is_empty() => {
            let prompt = rest.join(" ");
            aios::shell::ask(&prompt, false, false);
        }
        ["ask-private", rest @ ..] if !rest.is_empty() => {
            aios::shell::ask(&rest.join(" "), true, false);
        }
        ["monitor"] => aios::shell::monitor(),
        ["activity"] => aios::shell::activity(),
        ["model", "verify", manifest, file] => {
            match std::fs::read_to_string(manifest)
                .map_err(|e| e.to_string())
                .and_then(|j| libmodel::ModelManifest::from_json(&j))
            {
                Ok(man) => match man.verify_file(std::path::Path::new(file)) {
                    Ok(()) => println!(
                        "\u{2713} {} verified: checksum matches, trust={:?}",
                        man.model_id, man.trust_status
                    ),
                    Err(e) => {
                        eprintln!("\u{2717} verification FAILED: {e}");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("bad manifest: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => usage(),
    }
}
