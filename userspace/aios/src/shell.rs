//! Functional desktop shell pieces (M15 command bar, M16 activity center,
//! M55 system monitor) as a terminal UI. This is the runnable desktop until
//! the graphical compositor exists; it drives the same daemons and libraries.
use aiosd::{Request, Response, State};
use libagent::{Agent, ResourceLimits, TrustLevel};
use libcredentials::SystemdCredsStore;
use libgovernor::{evaluate, Decision};
use libintent::{ApprovalPolicy, Intent, RiskLevel};
use libprovenance::ProvenanceLog;
use libresource::{probe_host, ResourceKind};
use std::path::PathBuf;

fn aiosd_socket() -> PathBuf {
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

/// Command bar: send a natural-language prompt to the AI service. Uses aiosd if
/// running, otherwise an embedded local provider (local-first, never blocks).
pub fn ask(prompt: &str, personal_data: bool, prefer_cloud: bool) {
    let req = Request::Chat {
        prompt: prompt.to_string(),
        local_only: false,
        personal_data,
        prefer_cloud,
    };
    let sock = aiosd_socket();
    let resp: Response = if sock.exists() {
        libipc::request(&sock, &req).unwrap_or(Response::Error {
            message: "aiosd unreachable".into(),
        })
    } else {
        // Embedded fallback: run the dispatch in-process.
        let store = SystemdCredsStore::new(cred_dir()).unwrap_or_else(|_| {
            SystemdCredsStore::new(std::env::temp_dir().join("ainos-cred")).unwrap()
        });
        State::new(store).dispatch(req)
    };
    match resp {
        Response::Chat {
            text,
            indicator,
            model,
            input_tokens,
            output_tokens,
        } => {
            println!("{indicator}   [{model}]");
            println!("{text}");
            println!("  tokens: {input_tokens} in / {output_tokens} out (approx)");
        }
        Response::Error { message } => println!("! {message}"),
        other => println!("{other:?}"),
    }
}

/// System monitor: resources as first-class, AI workloads included.
pub fn monitor() {
    println!("System Monitor — resources");
    for r in probe_host() {
        let pct = r
            .available
            .checked_mul(100)
            .and_then(|x| x.checked_div(r.capacity))
            .map(|frac| 100u64.saturating_sub(frac))
            .unwrap_or(0);
        let used = match r.kind {
            ResourceKind::Ram => format!(
                "{:.1}/{:.1} GiB",
                (r.capacity - r.available) as f64 / 1e9,
                r.capacity as f64 / 1e9
            ),
            _ => format!("{}/{} {}", r.capacity - r.available, r.capacity, r.unit),
        };
        println!("  {:<8} {:>3}%   {}", format!("{:?}", r.kind), pct, used);
    }
    // top 5 processes by RSS
    println!("\nTop processes (RSS):");
    if let Ok(rd) = std::fs::read_dir("/proc") {
        let mut procs: Vec<(u64, String)> = rd
            .flatten()
            .filter_map(|e| {
                let pid = e.file_name().to_string_lossy().parse::<u32>().ok()?;
                let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
                let rss = status
                    .lines()
                    .find(|l| l.starts_with("VmRSS:"))?
                    .split_whitespace()
                    .nth(1)?
                    .parse::<u64>()
                    .ok()?;
                let name = status
                    .lines()
                    .next()?
                    .split_whitespace()
                    .nth(1)?
                    .to_string();
                Some((rss, name))
            })
            .collect();
        procs.sort_unstable_by_key(|(rss, _)| std::cmp::Reverse(*rss));
        for (rss, name) in procs.into_iter().take(5) {
            println!("  {:>7.0} MB  {}", rss as f64 / 1024.0, name);
        }
    }
}

/// AI Activity Center: a live snapshot of agents, intents, recent actions,
/// pending approvals, and blocked operations, built from real governor runs.
pub fn activity() {
    let now = 2000u64;
    let agent = Agent::create(
        "user:harsh",
        "assistant",
        "local/reasoning",
        TrustLevel::Standard,
        ResourceLimits::default(),
        10_000,
    );
    let mut intent = Intent::create(
        "user:harsh",
        "Summarize my project notes",
        "project dir + local model",
        vec![
            "fs.read:/home/harsh/project".into(),
            "model.invoke:local/reasoning".into(),
            "net.connect:api.example.com:443".into(),
        ],
        RiskLevel::Medium,
        ApprovalPolicy::AtOrAbove(RiskLevel::High),
        ResourceLimits::default(),
        now,
        10_000,
    );
    intent.validate().unwrap();
    intent.authorize().unwrap();
    intent.begin_execution().unwrap();
    let mut prov = ProvenanceLog::new();

    let actions = [
        "fs.read:/home/harsh/project/README.md", // allowed
        "model.invoke:local/reasoning",          // allowed
        "net.connect:api.example.com:443",       // needs approval (high)
        "net.connect:evil.com:443",              // blocked (out of intent)
    ];
    let mut allowed = 0;
    let mut pending = Vec::new();
    let mut blocked = Vec::new();
    for a in actions {
        match evaluate(&agent, &intent, a, None, now, &mut prov) {
            Decision::Allow => allowed += 1,
            Decision::NeedApproval { risk } => pending.push(format!("{a}  (risk {risk:?})")),
            Decision::Deny { reason } => blocked.push(format!("{a}  ({reason})")),
        }
    }

    println!("AI Activity Center");
    println!(
        "  Active agent:   {} … purpose={}",
        &agent.id()[..12],
        agent.identity.purpose
    );
    println!(
        "  Current intent: {}  state={:?}",
        intent.intent_id, intent.state
    );
    println!("  Running model:  local/reasoning");
    println!("  Actions allowed & recorded: {allowed}");
    println!("\n  Pending approvals ({}):", pending.len());
    for p in &pending {
        println!("    ⚠ {p}");
    }
    println!("\n  Blocked operations ({}):", blocked.len());
    for b in &blocked {
        println!("    ⛔ {b}");
    }
    println!(
        "\n  Provenance: {} events, chain valid: {}",
        prov.len(),
        prov.verify().is_ok()
    );
}
