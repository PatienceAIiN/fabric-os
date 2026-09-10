//! Fabric OS desktop — self-contained web desktop shell.
//! std-only HTTP/1.1 server + JSON API (auth, settings, account, agents/intents
//! CRUD, governor, provenance, monitor, AI). Serves the SPA at `/`.
#![allow(clippy::needless_return, clippy::too_many_lines)]

mod spa;
mod sys;

use libagent::{Agent, ResourceLimits, TrustLevel};
use libgovernor::evaluate;
use libintent::{ApprovalPolicy, Intent, RiskLevel};
use libprovenance::ProvenanceLog;
use libprovider::{AIProvider, ChatRequest, LlamaProvider, LocalProvider, Message};
use libresource::probe_host;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct AgentRow {
    agent: Agent,
    revoked: bool,
}
struct Creds {
    user: String,
    salt: String,
    hash: String,
}
struct AppState {
    agents: Vec<AgentRow>,
    intents: Vec<Intent>,
    prov: ProvenanceLog,
    sessions: HashMap<String, String>,
    settings: Value,
    creds: Creds,
    store: Value,
    keys: Value,
    dir: PathBuf,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
fn rand_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn hash_pw(salt: &str, pw: &str) -> String {
    libcrypto::hex(&libcrypto::hmac_sha256(salt.as_bytes(), pw.as_bytes()))
}

fn default_settings() -> Value {
    json!({ "theme":"light","accent":"#f26b3a","wallpaper":"white","font_scale":1.0,
            "reduce_motion":false,"system_wide_ai":true,"privacy":"local_only","default_model":"local/reasoning" })
}

fn state_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FABRIC_STATE") {
        return PathBuf::from(d);
    }
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/var/lib".into()))
                .join(".local/share")
        });
    base.join("fabric-os")
}

impl AppState {
    fn load() -> Self {
        let dir = state_dir();
        let _ = std::fs::create_dir_all(&dir);
        let settings = std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_settings);
        let creds = std::fs::read_to_string(dir.join("creds.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .map(|v| Creds {
                user: v["user"].as_str().unwrap_or("admin").into(),
                salt: v["salt"].as_str().unwrap_or("").into(),
                hash: v["hash"].as_str().unwrap_or("").into(),
            })
            .unwrap_or_else(|| {
                let salt = rand_hex(8);
                Creds {
                    hash: hash_pw(&salt, "fabric"),
                    salt,
                    user: "admin".into(),
                }
            });
        let mut store: Value = std::fs::read_to_string(dir.join("store.json")).ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({ "notifications": [], "clipboard": [], "workspaces": ["Workspace 1"], "bookmarks": [] }));
        let keys: Value = std::fs::read_to_string(dir.join("keys.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));
        if store
            .get("users")
            .and_then(|u| u.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true)
        {
            store["users"] = json!([{ "user": creds.user, "salt": creds.salt, "hash": creds.hash, "role": "admin" }]);
        }
        let s = AppState {
            agents: vec![],
            intents: vec![],
            prov: ProvenanceLog::new(),
            sessions: HashMap::new(),
            settings,
            creds,
            store,
            keys,
            dir,
        };
        s.save_creds();
        s.save_settings();
        s.save_store();
        s.save_keys();
        s
    }
    fn save_settings(&self) {
        let _ = std::fs::write(self.dir.join("settings.json"), self.settings.to_string());
    }
    fn save_keys(&self) {
        let p = self.dir.join("keys.json");
        let _ = std::fs::write(&p, self.keys.to_string());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
    fn verify_user(&self, user: &str, pw: &str) -> bool {
        self.store["users"]
            .as_array()
            .map(|a| {
                a.iter().any(|u| {
                    u["user"].as_str() == Some(user)
                        && u["hash"].as_str()
                            == Some(&hash_pw(u["salt"].as_str().unwrap_or(""), pw))
                })
            })
            .unwrap_or(false)
    }
    fn user_role(&self, user: &str) -> String {
        self.store["users"]
            .as_array()
            .and_then(|a| a.iter().find(|u| u["user"].as_str() == Some(user)))
            .and_then(|u| u["role"].as_str())
            .unwrap_or("user")
            .to_string()
    }
    fn save_store(&self) {
        let _ = std::fs::write(self.dir.join("store.json"), self.store.to_string());
    }
    fn save_creds(&self) {
        let v =
            json!({ "user": self.creds.user, "salt": self.creds.salt, "hash": self.creds.hash });
        let p = self.dir.join("creds.json");
        let _ = std::fs::write(&p, v.to_string());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

fn trust_of(s: &str) -> TrustLevel {
    match s {
        "untrusted" => TrustLevel::Untrusted,
        "standard" => TrustLevel::Standard,
        "elevated" => TrustLevel::Elevated,
        _ => TrustLevel::Limited,
    }
}
fn risk_of(s: &str) -> RiskLevel {
    match s {
        "low" => RiskLevel::Low,
        "high" => RiskLevel::High,
        "critical" => RiskLevel::Critical,
        _ => RiskLevel::Medium,
    }
}

struct Req {
    method: String,
    path: String,
    auth: Option<String>,
    body: String,
}
fn parse_req(stream: &mut TcpStream) -> Option<Req> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut it = line.split_whitespace();
    let method = it.next()?.to_string();
    let path = it.next()?.to_string();
    let mut clen = 0usize;
    let mut auth = None;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).ok()? == 0 {
            break;
        }
        let t = h.trim();
        if t.is_empty() {
            break;
        }
        let low = t.to_ascii_lowercase();
        if let Some(v) = low.strip_prefix("content-length:") {
            clen = v.trim().parse().unwrap_or(0);
        }
        if let Some(v) = t.strip_prefix("Authorization:") {
            auth = Some(v.trim().trim_start_matches("Bearer ").to_string());
        }
    }
    let mut body = vec![0u8; clen];
    if clen > 0 {
        reader.read_exact(&mut body).ok()?;
    }
    Some(Req {
        method,
        path,
        auth,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}
fn send(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) {
    let hdr = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n", body.len());
    let _ = stream.write_all(hdr.as_bytes());
    let _ = stream.write_all(body);
}
fn urldec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(n) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(n as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}
fn json_resp(stream: &mut TcpStream, status: &str, v: Value) {
    send(stream, status, "application/json", v.to_string().as_bytes());
}

fn agent_json(a: &AgentRow) -> Value {
    json!({ "id": a.agent.id(), "owner": a.agent.identity.owner, "purpose": a.agent.identity.purpose,
            "trust": format!("{:?}", a.agent.identity.trust_level), "created": a.agent.identity.creation_time,
            "expiration": a.agent.identity.expiration, "revoked": a.revoked })
}
fn intent_json(i: &Intent) -> Value {
    json!({ "id": i.intent_id, "objective": i.objective, "principal": i.principal, "caps": i.allowed_capabilities,
            "risk": format!("{:?}", i.risk_level), "state": format!("{:?}", i.state), "expiration": i.expiration })
}

fn bodyv(req: &Req) -> Value {
    serde_json::from_str(&req.body).unwrap_or(json!({}))
}
fn sv<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

/// Stateless routes that hit the REAL system (filesystem, /proc, processes).
fn sys_route(req: &Req) -> Option<(String, Value)> {
    let m = req.method.as_str();
    let p = req.path.as_str();
    let ok = |r: Result<Value, String>| {
        Some(match r {
            Ok(v) => ("200 OK".to_string(), v),
            Err(e) => ("400 Bad Request".to_string(), json!({"error": e})),
        })
    };
    match (m, p) {
        ("POST", "/api/fs/list") => ok(sys::fs_list(sv(&bodyv(req), "path"))),
        ("POST", "/api/fs/read") => ok(sys::fs_read(sv(&bodyv(req), "path"))),
        ("POST", "/api/fs/write") => {
            let v = bodyv(req);
            ok(sys::fs_write(sv(&v, "path"), sv(&v, "content")))
        }
        ("POST", "/api/fs/mkdir") => ok(sys::fs_mkdir(sv(&bodyv(req), "path"))),
        ("POST", "/api/fs/rename") => {
            let v = bodyv(req);
            ok(sys::fs_rename(sv(&v, "path"), sv(&v, "to")))
        }
        ("POST", "/api/fs/copy") => {
            let v = bodyv(req);
            ok(sys::fs_copy(sv(&v, "path"), sv(&v, "to")))
        }
        ("POST", "/api/fs/move") => {
            let v = bodyv(req);
            ok(sys::fs_move(sv(&v, "path"), sv(&v, "to")))
        }
        ("POST", "/api/fs/trash") => ok(sys::fs_trash(sv(&bodyv(req), "path"))),
        ("POST", "/api/fs/props") => ok(sys::fs_props(sv(&bodyv(req), "path"))),
        ("POST", "/api/fs/search") => {
            let v = bodyv(req);
            ok(sys::fs_search(sv(&v, "path"), sv(&v, "q")))
        }
        ("GET", "/api/trash") => ok(sys::trash_list()),
        ("POST", "/api/trash/restore") => ok(sys::trash_restore(sv(&bodyv(req), "id"))),
        ("POST", "/api/trash/empty") => {
            if bodyv(req)["confirm"].as_bool() != Some(true) {
                return Some((
                    "400 Bad Request".into(),
                    json!({"error":"confirmation required"}),
                ));
            }
            ok(sys::trash_empty())
        }
        ("GET", "/api/ps") => Some(("200 OK".into(), sys::ps_list())),
        ("POST", "/api/ps/kill") => {
            let v = bodyv(req);
            ok(sys::ps_kill(
                v["pid"].as_i64().unwrap_or(0) as i32,
                sv(&v, "sig"),
            ))
        }
        ("GET", "/api/sys/monitor") => Some(("200 OK".into(), sys::monitor())),
        ("GET", "/api/sys/info") => Some(("200 OK".into(), sys::sysinfo())),
        ("GET", "/api/sys/users") => Some(("200 OK".into(), sys::users())),
        ("GET", "/api/sys/storage") => Some(("200 OK".into(), sys::storage())),
        ("GET", "/api/sys/network") => Some(("200 OK".into(), sys::network())),
        ("POST", "/api/agent/plan") => {
            let v = bodyv(req);
            Some((
                "200 OK".into(),
                json!({ "steps": sys::agent_plan(sv(&v,"goal")) }),
            ))
        }
        ("POST", "/api/agent/run") => {
            let v = bodyv(req);
            Some((
                "200 OK".into(),
                sys::agent_run(sv(&v, "goal"), v["autonomous"].as_bool().unwrap_or(false)),
            ))
        }
        ("POST", "/api/exec") => {
            let v = bodyv(req);
            Some(("200 OK".into(), sys::exec(sv(&v, "cmd"), sv(&v, "cwd"))))
        }
        ("GET", "/api/apps") => Some(("200 OK".into(), sys::apps_installed())),
        ("GET", "/api/sys/updates") => Some(("200 OK".into(), sys::updates_check())),
        ("GET", "/api/sys/os-update") => Some(("200 OK".into(), sys::os_update_check())),
        ("POST", "/api/sys/updates/apply") => {
            if bodyv(req)["confirm"].as_bool() != Some(true) {
                return Some((
                    "400 Bad Request".into(),
                    json!({"error":"confirmation required"}),
                ));
            }
            ok(sys::updates_apply())
        }
        ("POST", "/api/apps/launch") => {
            let v = bodyv(req);
            ok(sys::app_launch(sv(&v, "exec")))
        }
        ("POST", "/api/apps/search") => {
            let v = bodyv(req);
            Some(("200 OK".into(), sys::app_search(sv(&v, "q"))))
        }
        ("POST", "/api/apps/install") => {
            let v = bodyv(req);
            if v["confirm"].as_bool() != Some(true) {
                return Some((
                    "400 Bad Request".into(),
                    json!({"error":"confirmation required"}),
                ));
            }
            ok(sys::app_install(sv(&v, "pkg")))
        }
        ("GET", "/api/sys/battery") => Some(("200 OK".into(), sys::battery())),
        ("GET", "/api/sys/datetime") => Some(("200 OK".into(), sys::datetime())),
        ("POST", "/api/sys/datetime") => {
            let v = bodyv(req);
            if let Some(iso) = v["time"].as_str() {
                return ok(sys::set_time(iso));
            }
            return ok(sys::set_ntp(v["ntp"].as_bool().unwrap_or(true)));
        }
        ("GET", "/api/sys/displays") => Some(("200 OK".into(), sys::displays())),
        ("GET", "/api/sys/wifi") => Some(("200 OK".into(), sys::wifi_list())),
        ("POST", "/api/sys/wifi/connect") => {
            let v = bodyv(req);
            ok(sys::wifi_connect(sv(&v, "ssid"), sv(&v, "pass")))
        }
        ("POST", "/api/sys/wifi/disconnect") => {
            let v = bodyv(req);
            ok(sys::wifi_disconnect(sv(&v, "ssid")))
        }
        ("POST", "/api/power") => {
            let v = bodyv(req);
            if v["confirm"].as_bool() != Some(true) {
                return Some((
                    "400 Bad Request".into(),
                    json!({"error":"confirmation required"}),
                ));
            }
            ok(sys::power(sv(&v, "action")))
        }
        _ => None,
    }
}

fn handle(state: &Arc<Mutex<AppState>>, req: &Req) -> (String, Value) {
    if req.method == "POST" && req.path == "/api/login" {
        let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
        let (u, p) = (
            v["username"].as_str().unwrap_or(""),
            v["password"].as_str().unwrap_or(""),
        );
        let mut s = state.lock().unwrap();
        if s.verify_user(u, p) {
            let t = rand_hex(16);
            s.sessions.insert(t.clone(), u.to_string());
            let role = s.user_role(u);
            return (
                "200 OK".into(),
                json!({ "token": t, "user": u, "role": role }),
            );
        }
        return (
            "401 Unauthorized".into(),
            json!({ "error": "invalid credentials" }),
        );
    }
    if req.method == "GET" && req.path == "/api/os/setup-status" {
        let s = state.lock().unwrap();
        return (
            "200 OK".into(),
            json!({ "done": s.settings.get("setup_done").and_then(|v| v.as_bool()).unwrap_or(false) }),
        );
    }
    if req.method == "GET" && req.path == "/api/os/userlist" {
        let s = state.lock().unwrap();
        let names: Vec<Value> = s.store["users"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|u| json!({"user":u["user"],"role":u["role"]}))
                    .collect()
            })
            .unwrap_or_default();
        return ("200 OK".into(), json!({ "users": names }));
    }
    if req.method == "POST" && req.path == "/api/os/forgot" {
        let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
        let target = v["user"].as_str().unwrap_or("").to_string();
        let mut s = state.lock().unwrap();
        let salt = rand_hex(8);
        let hash = hash_pw(&salt, "fabric");
        if let Some(a) = s.store["users"].as_array_mut() {
            for u in a.iter_mut() {
                if u["user"].as_str() == Some(&target) {
                    u["salt"] = json!(salt);
                    u["hash"] = json!(hash);
                }
            }
        }
        s.save_store();
        return ("200 OK".into(), json!({ "ok": true, "temp": "fabric" }));
    }
    let user = {
        let s = state.lock().unwrap();
        match req.auth.as_ref().and_then(|t| s.sessions.get(t)).cloned() {
            Some(u) => u,
            None => {
                return (
                    "401 Unauthorized".into(),
                    json!({ "error": "login required" }),
                )
            }
        }
    };
    if let Some(r) = sys_route(req) {
        return r;
    }
    let mut s = state.lock().unwrap();
    match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/api/logout") => {
            if let Some(t) = &req.auth {
                s.sessions.remove(t);
            }
            ("200 OK".into(), json!({"ok":true}))
        }
        ("GET", "/api/session") => ("200 OK".into(), json!({ "user": user })),
        ("GET", "/api/settings") => ("200 OK".into(), s.settings.clone()),
        ("PUT", "/api/settings") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            if let (Some(obj), Some(cur)) = (v.as_object(), s.settings.as_object_mut()) {
                for (k, val) in obj {
                    cur.insert(k.clone(), val.clone());
                }
            }
            s.save_settings();
            ("200 OK".into(), s.settings.clone())
        }
        ("POST", "/api/account/password") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let old = v["old"].as_str().unwrap_or("");
            let new = v["new"].as_str().unwrap_or("");
            if !s.verify_user(&user, old) {
                return (
                    "403 Forbidden".into(),
                    json!({"error":"current password incorrect"}),
                );
            }
            if new.len() < 3 {
                return (
                    "400 Bad Request".into(),
                    json!({"error":"new password too short"}),
                );
            }
            let salt = rand_hex(8);
            let hash = hash_pw(&salt, new);
            if let Some(a) = s.store["users"].as_array_mut() {
                for u in a.iter_mut() {
                    if u["user"].as_str() == Some(&user) {
                        u["salt"] = json!(salt);
                        u["hash"] = json!(hash);
                    }
                }
            }
            s.save_store();
            ("200 OK".into(), json!({"ok":true}))
        }
        ("POST", "/api/account/username") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let nu = v["username"].as_str().unwrap_or("").trim().to_string();
            if nu.is_empty() {
                return ("400 Bad Request".into(), json!({"error":"empty"}));
            }
            s.creds.user = nu.clone();
            s.save_creds();
            ("200 OK".into(), json!({"user": nu}))
        }
        ("GET", "/api/agents") => (
            "200 OK".into(),
            json!({ "agents": s.agents.iter().map(agent_json).collect::<Vec<_>>() }),
        ),
        ("POST", "/api/agents") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let a = Agent::create(
                v["owner"].as_str().unwrap_or("user:admin"),
                v["purpose"].as_str().unwrap_or("task"),
                "local/reasoning",
                trust_of(v["trust"].as_str().unwrap_or("limited")),
                ResourceLimits::default(),
                v["ttl"].as_u64().unwrap_or(3600),
            );
            let row = AgentRow {
                agent: a,
                revoked: false,
            };
            let out = agent_json(&row);
            s.agents.push(row);
            ("201 Created".into(), out)
        }
        ("GET", "/api/intents") => (
            "200 OK".into(),
            json!({ "intents": s.intents.iter().map(intent_json).collect::<Vec<_>>() }),
        ),
        ("POST", "/api/intents") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let caps: Vec<String> = v["caps"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let mut it = Intent::create(
                "user:admin",
                v["objective"].as_str().unwrap_or("(objective)"),
                v["scope"].as_str().unwrap_or("scoped"),
                caps,
                risk_of(v["risk"].as_str().unwrap_or("medium")),
                ApprovalPolicy::AtOrAbove(RiskLevel::High),
                ResourceLimits::default(),
                now(),
                v["ttl"].as_u64().unwrap_or(3600),
            );
            let _ = it.validate();
            let out = intent_json(&it);
            s.intents.push(it);
            ("201 Created".into(), out)
        }
        ("GET", "/api/provenance") => {
            let recs: Vec<Value> = s.prov.records().iter().map(|r| json!({ "seq": r.content.seq, "agent": r.content.agent_id,
                "intent": r.content.intent_id, "action": r.content.resource, "result": r.content.result,
                "decision": format!("{:?}", r.content.policy_decision), "ts": r.content.timestamp })).collect();
            (
                "200 OK".into(),
                json!({ "records": recs, "valid": s.prov.verify().is_ok() }),
            )
        }
        ("GET", "/api/monitor") => {
            let res: Vec<Value> = probe_host().iter().map(|r| json!({ "kind": format!("{:?}", r.kind), "capacity": r.capacity, "available": r.available, "unit": r.unit })).collect();
            (
                "200 OK".into(),
                json!({ "resources": res, "agents": s.agents.iter().filter(|a|!a.revoked).count(), "intents": s.intents.len() }),
            )
        }
        ("POST", "/api/ask") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let prompt = v["prompt"].as_str().unwrap_or("").to_string();
            let want_cloud = s.settings["privacy"].as_str() == Some("allow_cloud")
                && s.keys.get("cloud").is_some();
            if want_cloud {
                let ep = s.settings["cloud_endpoint"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let model = s.settings["cloud_model"]
                    .as_str()
                    .unwrap_or("gpt-4o-mini")
                    .to_string();
                let key = s.keys["cloud"].as_str().unwrap_or("").to_string();
                if let Ok(t) = sys::cloud_chat(&ep, &model, &key, &prompt) {
                    return (
                        "200 OK".into(),
                        json!({ "text": t, "backend": format!("cloud · {}", s.settings["cloud_name"].as_str().unwrap_or("provider")) }),
                    );
                }
            }
            let cr = ChatRequest {
                model: "local".into(),
                system: None,
                messages: vec![Message {
                    role: "user".into(),
                    content: prompt,
                }],
                max_tokens: 220,
            };
            let (text, ind) = if let Some(l) = LlamaProvider::from_env() {
                match l.chat(&cr) {
                    Ok(r) => {
                        let gpu = std::env::var("AIOS_LLAMA_NGL")
                            .ok()
                            .as_deref()
                            .unwrap_or("0")
                            != "0";
                        (
                            r.text,
                            if gpu {
                                "GPU (llama.cpp)"
                            } else {
                                "CPU (llama.cpp)"
                            },
                        )
                    }
                    Err(_) => (LocalProvider::new().chat(&cr).unwrap().text, "stub"),
                }
            } else {
                (LocalProvider::new().chat(&cr).unwrap().text, "stub")
            };
            ("200 OK".into(), json!({ "text": text, "backend": ind }))
        }
        (m, p) if p.starts_with("/api/agents/") => {
            let id = p.trim_start_matches("/api/agents/");
            if m == "DELETE" {
                if let Some(row) = s.agents.iter_mut().find(|a| a.agent.id() == id) {
                    row.revoked = true;
                    return ("200 OK".into(), json!({ "revoked": id }));
                }
                ("404 Not Found".into(), json!({ "error": "no such agent" }))
            } else {
                ("405 Method Not Allowed".into(), json!({"error":"method"}))
            }
        }
        (m, p) if p.starts_with("/api/intents/") => {
            let rest = p.trim_start_matches("/api/intents/");
            let (id, action) = rest.split_once('/').unwrap_or((rest, ""));
            let idx = s.intents.iter().position(|i| i.intent_id == id);
            match (m, action) {
                ("POST", "authorize") => {
                    if let Some(i) = idx {
                        let _ = s.intents[i].authorize();
                        let _ = s.intents[i].begin_execution();
                        return ("200 OK".into(), intent_json(&s.intents[i]));
                    }
                    ("404 Not Found".into(), json!({"error":"no intent"}))
                }
                ("DELETE", "") => {
                    if let Some(i) = idx {
                        let _ = s.intents[i].cancel();
                        let out = intent_json(&s.intents[i]);
                        return ("200 OK".into(), out);
                    }
                    ("404 Not Found".into(), json!({"error":"no intent"}))
                }
                _ => ("405 Method Not Allowed".into(), json!({"error":"method"})),
            }
        }
        ("POST", "/api/evaluate") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let aid = v["agent_id"].as_str().unwrap_or("");
            let iid = v["intent_id"].as_str().unwrap_or("");
            let capr = v["capability"].as_str().unwrap_or("");
            let agent = s
                .agents
                .iter()
                .find(|a| a.agent.id() == aid && !a.revoked)
                .map(|a| a.agent.clone());
            let intent = s.intents.iter().find(|i| i.intent_id == iid).cloned();
            match (agent, intent) {
                (Some(a), Some(i)) => {
                    let d = evaluate(&a, &i, capr, None, now(), &mut s.prov);
                    ("200 OK".into(), json!({ "decision": format!("{:?}", d) }))
                }
                _ => (
                    "400 Bad Request".into(),
                    json!({ "error": "unknown/revoked agent or intent" }),
                ),
            }
        }
        ("POST", "/api/feedback") | ("POST", "/api/error-report") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let kind = if req.path.ends_with("error-report") {
                "error"
            } else {
                "feedback"
            };
            let subject = format!(
                "[Fabric OS {}] {}",
                kind,
                v["subject"].as_str().unwrap_or("report")
            );
            let body = format!(
                "From user: {}\nType: {}\n\n{}\n\n---\n{}",
                user,
                v["type"].as_str().unwrap_or(kind),
                v["message"].as_str().unwrap_or(""),
                v["detail"].as_str().unwrap_or("")
            );
            // always log locally
            let logline = format!(
                "{} {} {}: {}\n",
                now(),
                kind,
                user,
                v["message"].as_str().unwrap_or("")
            );
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(s.dir.join("feedback.log"))
                .and_then(|mut f| {
                    use std::io::Write;
                    f.write_all(logline.as_bytes())
                });
            // try to email Patience AI if SMTP configured
            let host = s.settings["smtp_host"].as_str().unwrap_or("").to_string();
            let port = s.settings["smtp_port"].as_u64().unwrap_or(465) as u16;
            let smu = s.settings["smtp_user"].as_str().unwrap_or("").to_string();
            let from = s.settings["smtp_from"].as_str().unwrap_or(&smu).to_string();
            let pass = s.keys["smtp"].as_str().unwrap_or("").to_string();
            if !host.is_empty() {
                match sys::send_mail(
                    &host,
                    port,
                    &smu,
                    &pass,
                    &from,
                    "info@patienceai.in",
                    &subject,
                    &body,
                ) {
                    Ok(_) => ("200 OK".into(), json!({"ok":true,"sent":true})),
                    Err(e) => (
                        "200 OK".into(),
                        json!({"ok":true,"sent":false,"note":format!("saved locally; email failed: {e}")}),
                    ),
                }
            } else {
                (
                    "200 OK".into(),
                    json!({"ok":true,"sent":false,"note":"saved locally (configure Settings → Mail to email Patience AI)"}),
                )
            }
        }
        // ---- OS users (app-level, hashed) ----
        ("GET", "/api/os/users") => (
            "200 OK".into(),
            json!({ "users": s.store["users"].as_array().map(|a| a.iter().map(|u| json!({"user":u["user"],"role":u["role"]})).collect::<Vec<_>>()).unwrap_or_default(), "me": user, "role": s.user_role(&user) }),
        ),
        ("POST", "/api/os/users") => {
            if s.user_role(&user) != "admin" {
                return ("403 Forbidden".into(), json!({"error":"admin only"}));
            }
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let nu = v["user"].as_str().unwrap_or("").trim().to_string();
            let pw = v["pass"].as_str().unwrap_or("");
            if nu.is_empty() || pw.len() < 3 {
                return (
                    "400 Bad Request".into(),
                    json!({"error":"username + password (3+) required"}),
                );
            }
            if s.store["users"]
                .as_array()
                .map(|a| a.iter().any(|u| u["user"].as_str() == Some(&nu)))
                .unwrap_or(false)
            {
                return ("400 Bad Request".into(), json!({"error":"user exists"}));
            }
            let salt = rand_hex(8);
            if let Some(a) = s.store["users"].as_array_mut() {
                a.push(json!({"user":nu,"salt":salt,"hash":hash_pw(&salt,pw),"role":v["role"].as_str().unwrap_or("user")}));
            }
            s.save_store();
            ("201 Created".into(), json!({"user":nu}))
        }
        (m, p) if p.starts_with("/api/os/users/") => {
            let rest = p.trim_start_matches("/api/os/users/");
            let (name, action) = rest.split_once('/').unwrap_or((rest, ""));
            if s.user_role(&user) != "admin" && name != user {
                return ("403 Forbidden".into(), json!({"error":"not allowed"}));
            }
            if m == "DELETE" {
                if name == "admin" {
                    return (
                        "400 Bad Request".into(),
                        json!({"error":"cannot delete admin"}),
                    );
                }
                if let Some(a) = s.store["users"].as_array_mut() {
                    a.retain(|u| u["user"].as_str() != Some(name));
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else if m == "POST" && action == "reset" {
                let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
                let np = v["new"].as_str().unwrap_or("");
                if np.len() < 3 {
                    return (
                        "400 Bad Request".into(),
                        json!({"error":"password too short"}),
                    );
                }
                let salt = rand_hex(8);
                if let Some(a) = s.store["users"].as_array_mut() {
                    for u in a.iter_mut() {
                        if u["user"].as_str() == Some(name) {
                            u["salt"] = json!(salt);
                            u["hash"] = json!(hash_pw(&salt, np));
                        }
                    }
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else {
                ("405 Method Not Allowed".into(), json!({"error":"m"}))
            }
        }
        // ---- cloud AI provider config ----
        ("GET", "/api/ai/provider") => (
            "200 OK".into(),
            json!({ "name": s.settings["cloud_name"], "endpoint": s.settings["cloud_endpoint"], "model": s.settings["cloud_model"], "configured": s.keys.get("cloud").is_some() }),
        ),
        ("POST", "/api/ai/provider") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            if let Some(o) = s.settings.as_object_mut() {
                o.insert("cloud_name".into(), v["name"].clone());
                o.insert("cloud_endpoint".into(), v["endpoint"].clone());
                o.insert("cloud_model".into(), v["model"].clone());
            }
            if let Some(k) = v["key"].as_str() {
                if !k.is_empty() {
                    s.keys["cloud"] = json!(k);
                }
            }
            s.save_settings();
            s.save_keys();
            ("200 OK".into(), json!({"ok":true}))
        }
        // ---- mail ----
        ("GET", "/api/mail/config") => (
            "200 OK".into(),
            json!({ "host": s.settings["smtp_host"], "port": s.settings["smtp_port"], "user": s.settings["smtp_user"], "from": s.settings["smtp_from"], "configured": s.keys.get("smtp").is_some() }),
        ),
        ("POST", "/api/mail/config") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            if let Some(o) = s.settings.as_object_mut() {
                for k in ["smtp_host", "smtp_user", "smtp_from"] {
                    o.insert(k.into(), v[k.trim_start_matches("smtp_")].clone());
                }
                o.insert("smtp_port".into(), json!(v["port"].as_u64().unwrap_or(465)));
            }
            if let Some(pw) = v["pass"].as_str() {
                if !pw.is_empty() {
                    s.keys["smtp"] = json!(pw);
                }
            }
            s.save_settings();
            s.save_keys();
            ("200 OK".into(), json!({"ok":true}))
        }
        ("POST", "/api/mail/send") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let host = s.settings["smtp_host"].as_str().unwrap_or("").to_string();
            let port = s.settings["smtp_port"].as_u64().unwrap_or(465) as u16;
            let smu = s.settings["smtp_user"].as_str().unwrap_or("").to_string();
            let from = s.settings["smtp_from"].as_str().unwrap_or(&smu).to_string();
            let pass = s.keys["smtp"].as_str().unwrap_or("").to_string();
            match sys::send_mail(
                &host,
                port,
                &smu,
                &pass,
                &from,
                v["to"].as_str().unwrap_or(""),
                v["subject"].as_str().unwrap_or("(no subject)"),
                v["body"].as_str().unwrap_or(""),
            ) {
                Ok(r) => ("200 OK".into(), r),
                Err(e) => ("400 Bad Request".into(), json!({"error":e})),
            }
        }
        // ---- wallpaper upload (base64 data URL) ----
        ("POST", "/api/wallpaper") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            if let Some(data) = v["data"].as_str() {
                let _ = std::fs::write(s.dir.join("wallpaper.dat"), data);
                if let Some(o) = s.settings.as_object_mut() {
                    o.insert("wallpaper".into(), json!("custom"));
                }
                s.save_settings();
                ("200 OK".into(), json!({"ok":true}))
            } else {
                ("400 Bad Request".into(), json!({"error":"no data"}))
            }
        }
        ("GET", "/api/wallpaper") => match std::fs::read_to_string(s.dir.join("wallpaper.dat")) {
            Ok(d) => ("200 OK".into(), json!({"data":d})),
            Err(_) => ("404 Not Found".into(), json!({"error":"none"})),
        },
        // ---- notifications / clipboard / workspaces / bookmarks (persisted CRUD) ----
        ("GET", "/api/notifications") => (
            "200 OK".into(),
            json!({ "items": s.store["notifications"] }),
        ),
        ("POST", "/api/notifications") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let n = json!({ "id": rand_hex(6), "title": v["title"].as_str().unwrap_or("Notice"), "body": v["body"].as_str().unwrap_or(""), "ts": now(), "read": false });
            if let Some(a) = s.store["notifications"].as_array_mut() {
                a.insert(0, n.clone());
                a.truncate(100);
            }
            s.save_store();
            ("201 Created".into(), n)
        }
        (m, p) if p.starts_with("/api/notifications/") => {
            let id = p.trim_start_matches("/api/notifications/").to_string();
            if m == "DELETE" {
                if let Some(a) = s.store["notifications"].as_array_mut() {
                    a.retain(|x| x["id"].as_str() != Some(&id));
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else if m == "POST" {
                if let Some(a) = s.store["notifications"].as_array_mut() {
                    for x in a.iter_mut() {
                        if x["id"].as_str() == Some(&id) {
                            x["read"] = json!(true);
                        }
                    }
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else {
                ("405 Method Not Allowed".into(), json!({"error":"method"}))
            }
        }
        ("DELETE", "/api/notifications") => {
            s.store["notifications"] = json!([]);
            s.save_store();
            ("200 OK".into(), json!({"ok":true}))
        }
        ("GET", "/api/clipboard") => ("200 OK".into(), json!({ "items": s.store["clipboard"] })),
        ("POST", "/api/clipboard") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let c =
                json!({ "id": rand_hex(6), "text": v["text"].as_str().unwrap_or(""), "ts": now() });
            if let Some(a) = s.store["clipboard"].as_array_mut() {
                a.insert(0, c.clone());
                a.truncate(50);
            }
            s.save_store();
            ("201 Created".into(), c)
        }
        (m, p) if p.starts_with("/api/clipboard/") => {
            let id = p.trim_start_matches("/api/clipboard/").to_string();
            if m == "DELETE" {
                if let Some(a) = s.store["clipboard"].as_array_mut() {
                    a.retain(|x| x["id"].as_str() != Some(&id));
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else {
                ("405 Method Not Allowed".into(), json!({"error":"m"}))
            }
        }
        ("GET", "/api/workspaces") => ("200 OK".into(), json!({ "items": s.store["workspaces"] })),
        ("POST", "/api/workspaces") => {
            let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
            let name = v["name"].as_str().unwrap_or("Workspace").to_string();
            if let Some(a) = s.store["workspaces"].as_array_mut() {
                a.push(json!(name));
            }
            s.save_store();
            ("201 Created".into(), json!({"name":name}))
        }
        (m, p) if p.starts_with("/api/workspaces/") => {
            let idx: usize = p
                .trim_start_matches("/api/workspaces/")
                .parse()
                .unwrap_or(999);
            if m == "DELETE" {
                if let Some(a) = s.store["workspaces"].as_array_mut() {
                    if idx < a.len() && a.len() > 1 {
                        a.remove(idx);
                    }
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else if m == "PUT" {
                let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
                if let Some(a) = s.store["workspaces"].as_array_mut() {
                    if idx < a.len() {
                        a[idx] = json!(v["name"].as_str().unwrap_or("Workspace"));
                    }
                }
                s.save_store();
                ("200 OK".into(), json!({"ok":true}))
            } else {
                ("405 Method Not Allowed".into(), json!({"error":"m"}))
            }
        }
        _ => ("404 Not Found".into(), json!({ "error": "not found" })),
    }
}

fn main() {
    let addr = std::env::var("FABRIC_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
    sys::seed_root();
    let state = Arc::new(Mutex::new(AppState::load()));
    let listener = TcpListener::bind(&addr).expect("bind");
    eprintln!("Fabric OS desktop on http://{addr}  (login: admin / fabric)");
    for conn in listener.incoming() {
        let Ok(mut stream) = conn else { continue };
        let state = state.clone();
        std::thread::spawn(move || {
            if let Some(req) = parse_req(&mut stream) {
                if req.method == "GET" && (req.path == "/" || req.path == "/index.html") {
                    send(
                        &mut stream,
                        "200 OK",
                        "text/html; charset=utf-8",
                        spa::INDEX_HTML.as_bytes(),
                    );
                } else if req.method == "GET" && req.path == "/favicon.svg" {
                    send(
                        &mut stream,
                        "200 OK",
                        "image/svg+xml",
                        spa::FAVICON.as_bytes(),
                    );
                } else if req.method == "GET" && req.path.starts_with("/api/raw?") {
                    let qs = &req.path[9..];
                    let mut path = String::new();
                    let mut tok = String::new();
                    for kv in qs.split('&') {
                        if let Some((k, v)) = kv.split_once('=') {
                            let v = urldec(v);
                            if k == "path" {
                                path = v;
                            } else if k == "token" {
                                tok = v;
                            }
                        }
                    }
                    let authed = { state.lock().unwrap().sessions.contains_key(&tok) };
                    if !authed {
                        send(
                            &mut stream,
                            "401 Unauthorized",
                            "text/plain",
                            b"login required",
                        );
                    } else if let Some(p) = sys::confine(&path) {
                        match std::fs::read(&p) {
                            Ok(bytes) => {
                                let ct = match p.extension().and_then(|e| e.to_str()).unwrap_or("")
                                {
                                    "png" => "image/png",
                                    "jpg" | "jpeg" => "image/jpeg",
                                    "gif" => "image/gif",
                                    "svg" => "image/svg+xml",
                                    "webp" => "image/webp",
                                    _ => "application/octet-stream",
                                };
                                send(&mut stream, "200 OK", ct, &bytes);
                            }
                            Err(_) => {
                                send(&mut stream, "404 Not Found", "text/plain", b"not found")
                            }
                        }
                    } else {
                        send(&mut stream, "403 Forbidden", "text/plain", b"denied");
                    }
                } else if req.path.starts_with("/api/") {
                    let (status, body) = handle(&state, &req);
                    json_resp(&mut stream, &status, body);
                } else {
                    send(&mut stream, "404 Not Found", "text/plain", b"not found");
                }
            }
        });
    }
}
