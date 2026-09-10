//! Fabric OS real system services. Every function performs a genuine operation
//! against the running Linux system (filesystem, /proc, processes, commands),
//! confined and validated. No fake data.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The filesystem sandbox root. Real files live here; operations are confined
/// to it (path-traversal protected). Override with FABRIC_ROOT.
pub fn fs_root() -> PathBuf {
    if let Ok(r) = std::env::var("FABRIC_ROOT") {
        return PathBuf::from(r);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join("FabricOS")
}

/// Seed the workspace on first run with real folders + a welcome file.
pub fn seed_root() {
    let root = fs_root();
    for d in [
        "Documents",
        "Downloads",
        "Pictures",
        "Music",
        "Videos",
        ".Trash",
    ] {
        let _ = std::fs::create_dir_all(root.join(d));
    }
    let w = root.join("Documents/Welcome.txt");
    if !w.exists() {
        let _ = std::fs::write(
            &w,
            "Welcome to Fabric OS.\n\nThis is a real file on disk. Create, edit, rename,\ntrash and restore files — every action is real.\n",
        );
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Confine a relative/absolute path to the sandbox root. Rejects `..` escapes.
pub fn confine(rel: &str) -> Option<PathBuf> {
    let root = normalize(&fs_root());
    // UI paths are workspace-relative; a leading '/' means the workspace root.
    let rel = rel.trim_start_matches('/');
    let n = normalize(&root.join(rel));
    if n == root || n.starts_with(&root) {
        Some(n)
    } else {
        None
    }
}
fn normalize(p: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for c in p.components() {
        use std::path::Component::*;
        match c {
            Prefix(_) | RootDir => out.clear(),
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            Normal(x) => out.push(x.to_os_string()),
        }
    }
    let mut abs = PathBuf::from("/");
    for c in &out {
        abs.push(c);
    }
    abs
}
fn rel_of(p: &Path) -> String {
    let root = normalize(&fs_root());
    p.strip_prefix(&root)
        .map(|r| format!("/{}", r.display()))
        .unwrap_or_else(|_| "/".into())
        .replace("//", "/")
}

fn meta_json(name: &str, p: &Path) -> Value {
    let m = std::fs::symlink_metadata(p).ok();
    let (is_dir, size, mtime) = m
        .as_ref()
        .map(|m| {
            (
                m.is_dir(),
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            )
        })
        .unwrap_or((false, 0, 0));
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    json!({ "name": name, "path": rel_of(p), "dir": is_dir, "size": size, "mtime": mtime, "ext": ext })
}

// ---------------- Filesystem CRUD (real) ----------------
pub fn fs_list(rel: &str) -> Result<Value, String> {
    let p = confine(rel).ok_or("path not allowed")?;
    let rd = std::fs::read_dir(&p).map_err(|e| e.to_string())?;
    let mut items: Vec<Value> = rd
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with(".Trash"))
        .map(|e| meta_json(&e.file_name().to_string_lossy(), &e.path()))
        .collect();
    items.sort_by(|a, b| {
        (b["dir"].as_bool(), a["name"].as_str()).cmp(&(a["dir"].as_bool(), b["name"].as_str()))
    });
    Ok(json!({ "path": rel_of(&p), "items": items }))
}
pub fn fs_read(rel: &str) -> Result<Value, String> {
    let p = confine(rel).ok_or("path not allowed")?;
    let md = std::fs::metadata(&p).map_err(|e| e.to_string())?;
    if md.len() > 2_000_000 {
        return Err("file too large to open (>2MB)".into());
    }
    let content = std::fs::read_to_string(&p).map_err(|e| format!("cannot read (binary?): {e}"))?;
    Ok(json!({ "path": rel_of(&p), "content": content }))
}
pub fn fs_write(rel: &str, content: &str) -> Result<Value, String> {
    let p = confine(rel).ok_or("path not allowed")?;
    std::fs::write(&p, content).map_err(|e| e.to_string())?;
    Ok(json!({ "path": rel_of(&p), "ok": true }))
}
pub fn fs_mkdir(rel: &str) -> Result<Value, String> {
    let p = confine(rel).ok_or("path not allowed")?;
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(json!({ "path": rel_of(&p), "ok": true }))
}
pub fn fs_rename(rel: &str, to: &str) -> Result<Value, String> {
    let a = confine(rel).ok_or("path not allowed")?;
    let parent = a.parent().unwrap_or(Path::new("/"));
    let b = confine(&rel_of(&parent.join(to))).ok_or("target not allowed")?;
    std::fs::rename(&a, &b).map_err(|e| e.to_string())?;
    Ok(json!({ "path": rel_of(&b), "ok": true }))
}
pub fn fs_copy(rel: &str, to_dir: &str) -> Result<Value, String> {
    let a = confine(rel).ok_or("src not allowed")?;
    let name = a
        .file_name()
        .ok_or("bad name")?
        .to_string_lossy()
        .to_string();
    let dst = confine(to_dir).ok_or("dst not allowed")?.join(&name);
    if a.is_dir() {
        copy_dir(&a, &dst).map_err(|e| e.to_string())?;
    } else {
        std::fs::copy(&a, &dst).map_err(|e| e.to_string())?;
    }
    Ok(json!({ "path": rel_of(&dst), "ok": true }))
}
fn copy_dir(a: &Path, b: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(b)?;
    for e in std::fs::read_dir(a)? {
        let e = e?;
        let t = b.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &t)?;
        } else {
            std::fs::copy(e.path(), t)?;
        }
    }
    Ok(())
}
pub fn fs_move(rel: &str, to_dir: &str) -> Result<Value, String> {
    let a = confine(rel).ok_or("src not allowed")?;
    let name = a
        .file_name()
        .ok_or("bad name")?
        .to_string_lossy()
        .to_string();
    let dst = confine(to_dir).ok_or("dst not allowed")?.join(&name);
    std::fs::rename(&a, &dst).map_err(|e| e.to_string())?;
    Ok(json!({ "path": rel_of(&dst), "ok": true }))
}
/// Move to trash (never a permanent delete).
pub fn fs_trash(rel: &str) -> Result<Value, String> {
    let a = confine(rel).ok_or("path not allowed")?;
    let name = a
        .file_name()
        .ok_or("bad name")?
        .to_string_lossy()
        .to_string();
    let trash = fs_root().join(".Trash");
    std::fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
    let stamped = format!("{}__{}", now(), name);
    std::fs::rename(&a, trash.join(&stamped)).map_err(|e| e.to_string())?;
    // remember origin
    let idx = trash.join(".index");
    let mut map: Value = std::fs::read_to_string(&idx)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    map[&stamped] = json!(rel_of(&a));
    let _ = std::fs::write(&idx, map.to_string());
    Ok(json!({ "ok": true }))
}
pub fn trash_list() -> Result<Value, String> {
    let trash = fs_root().join(".Trash");
    let idx: Value = std::fs::read_to_string(trash.join(".index"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    let items: Vec<Value> = std::fs::read_dir(&trash).map(|rd| rd.flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| { let n = e.file_name().to_string_lossy().to_string();
            let orig = idx.get(&n).and_then(|v| v.as_str()).unwrap_or("").to_string();
            json!({ "name": n.split_once("__").map(|x| x.1).unwrap_or(&n), "id": n, "origin": orig, "dir": e.path().is_dir() }) }).collect())
        .unwrap_or_default();
    Ok(json!({ "items": items }))
}
pub fn trash_restore(id: &str) -> Result<Value, String> {
    let trash = fs_root().join(".Trash");
    let idx_path = trash.join(".index");
    let mut idx: Value = std::fs::read_to_string(&idx_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    let origin = idx
        .get(id)
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("/{}", id.split_once("__").map(|x| x.1).unwrap_or(id)));
    let dst = confine(&origin).ok_or("origin not allowed")?;
    std::fs::rename(trash.join(id), &dst).map_err(|e| e.to_string())?;
    if let Some(o) = idx.as_object_mut() {
        o.remove(id);
    }
    let _ = std::fs::write(&idx_path, idx.to_string());
    Ok(json!({ "ok": true, "path": rel_of(&dst) }))
}
pub fn trash_empty() -> Result<Value, String> {
    let trash = fs_root().join(".Trash");
    let _ = std::fs::remove_dir_all(&trash);
    std::fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true }))
}
pub fn fs_props(rel: &str) -> Result<Value, String> {
    let p = confine(rel).ok_or("path not allowed")?;
    let m = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    let (mode, uid, gid) = {
        #[cfg(unix)]
        {
            (m.mode(), m.uid(), m.gid())
        }
        #[cfg(not(unix))]
        {
            (0u32, 0u32, 0u32)
        }
    };
    Ok(
        json!({ "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "path": rel_of(&p), "dir": m.is_dir(), "size": m.len(),
        "mode": format!("{:o}", mode & 0o777), "uid": uid, "gid": gid,
        "modified": m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0) }),
    )
}
pub fn fs_search(root_rel: &str, query: &str) -> Result<Value, String> {
    let start = confine(root_rel).ok_or("path not allowed")?;
    let q = query.to_lowercase();
    let mut hits = Vec::new();
    fn walk(dir: &Path, q: &str, hits: &mut Vec<Value>, root: &Path) {
        if hits.len() > 300 {
            return;
        }
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(".Trash") {
                    continue;
                }
                if name.to_lowercase().contains(q) {
                    let rp = e
                        .path()
                        .strip_prefix(root)
                        .map(|r| format!("/{}", r.display()))
                        .unwrap_or_default();
                    hits.push(json!({ "name": name, "path": rp, "dir": e.path().is_dir() }));
                }
                if e.path().is_dir() {
                    walk(&e.path(), q, hits, root);
                }
            }
        }
    }
    let root = normalize(&fs_root());
    walk(&start, &q, &mut hits, &root);
    Ok(json!({ "items": hits }))
}

// ---------------- Processes (real /proc) ----------------
pub fn ps_list() -> Value {
    let mut procs = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/proc") {
        for e in rd.flatten() {
            let pid = match e.file_name().to_string_lossy().parse::<i32>() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let status = match std::fs::read_to_string(format!("/proc/{pid}/status")) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let get = |k: &str| {
                status
                    .lines()
                    .find(|l| l.starts_with(k))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|v| v.trim().to_string())
                    .unwrap_or_default()
            };
            let name = get("Name");
            let rss_kb = get("VmRSS")
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let state = get("State");
            let uid = get("Uid")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            let cmd = std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
                .ok()
                .map(|c| c.replace('\0', " ").trim().to_string())
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| format!("[{name}]"));
            procs.push(json!({ "pid": pid, "name": name, "rss_mb": rss_kb/1024, "state": state, "uid": uid, "cmd": cmd }));
        }
    }
    procs.sort_by(|a, b| b["rss_mb"].as_u64().cmp(&a["rss_mb"].as_u64()));
    procs.truncate(200);
    json!({ "processes": procs })
}
pub fn ps_kill(pid: i32, sig: &str) -> Result<Value, String> {
    if pid <= 1 {
        return Err("refusing to signal pid <= 1".into());
    }
    let s = match sig {
        "KILL" => "-KILL",
        "STOP" => "-STOP",
        "CONT" => "-CONT",
        _ => "-TERM",
    };
    let out = Command::new("kill")
        .args([s, &pid.to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({ "ok": true }))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------- Monitor (real metrics) ----------------
fn read_stat_busy() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let l = s.lines().next().unwrap_or("");
    let v: Vec<u64> = l
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
    let total: u64 = v.iter().sum();
    (total.saturating_sub(idle), total)
}
pub fn monitor() -> Value {
    let (b1, t1) = read_stat_busy();
    std::thread::sleep(std::time::Duration::from_millis(180));
    let (b2, t2) = read_stat_busy();
    let cpu = if t2 > t1 {
        ((b2 - b1) as f64 / (t2 - t1) as f64 * 100.0).round()
    } else {
        0.0
    };
    let mem = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mget = |k: &str| {
        mem.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
    };
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let la: Vec<&str> = load.split_whitespace().take(3).collect();
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        })
        .unwrap_or(0.0);
    let gpu = std::fs::read_to_string("/sys/class/drm/card1/device/gpu_busy_percent")
        .ok()
        .or_else(|| std::fs::read_to_string("/sys/class/drm/card0/device/gpu_busy_percent").ok())
        .and_then(|s| s.trim().parse::<f64>().ok());
    json!({ "cpu": cpu, "mem_total": mget("MemTotal:"), "mem_avail": mget("MemAvailable:"),
            "swap_total": mget("SwapTotal:"), "swap_free": mget("SwapFree:"),
            "load": la, "uptime": uptime, "gpu": gpu })
}

// ---------------- System info (real) ----------------
pub fn sysinfo() -> Value {
    let uname = |a: &str| {
        Command::new("uname")
            .arg(a)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };
    let osrel = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let pretty = osrel
        .lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| {
            l.trim_start_matches("PRETTY_NAME=")
                .trim_matches('"')
                .to_string()
        })
        .unwrap_or_else(|| "Fabric OS".into());
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let cores = cpuinfo
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count();
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    json!({ "os": pretty, "kernel": uname("-r"), "arch": uname("-m"), "hostname": host,
            "cpu": model, "cores": cores })
}

// ---------------- Users / storage / network (real read) ----------------
pub fn users() -> Value {
    let pw = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let list: Vec<Value> = pw.lines().filter_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        if f.len() < 7 { return None; }
        let uid: u32 = f[2].parse().ok()?;
        Some(json!({ "user": f[0], "uid": uid, "home": f[5], "shell": f[6], "system": uid < 1000 }))
    }).collect();
    json!({ "users": list })
}
pub fn storage() -> Value {
    let out = Command::new("df")
        .args(["-B1", "--output=source,fstype,size,used,avail,target"])
        .output()
        .ok();
    let mut disks = Vec::new();
    if let Some(o) = out {
        for l in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
            let c: Vec<&str> = l.split_whitespace().collect();
            if c.len() < 6 {
                continue;
            }
            if !c[0].starts_with("/dev") && c[1] != "tmpfs" {
                continue;
            }
            disks.push(json!({ "source": c[0], "fstype": c[1], "size": c[2].parse::<u64>().unwrap_or(0),
                "used": c[3].parse::<u64>().unwrap_or(0), "avail": c[4].parse::<u64>().unwrap_or(0), "mount": c[5] }));
        }
    }
    json!({ "disks": disks })
}
pub fn network() -> Value {
    let mut ifs = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/class/net") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let oper = std::fs::read_to_string(e.path().join("operstate"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let mac = std::fs::read_to_string(e.path().join("address"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let addr = Command::new("ip")
                .args(["-o", "-4", "addr", "show", &name])
                .output()
                .ok()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .split_whitespace()
                        .skip_while(|w| *w != "inet")
                        .nth(1)
                        .unwrap_or("")
                        .to_string()
                })
                .unwrap_or_default();
            ifs.push(json!({ "name": name, "state": oper, "mac": mac, "ipv4": addr }));
        }
    }
    json!({ "interfaces": ifs })
}

// ---------------- Terminal (real command exec) ----------------
pub fn exec(cmd: &str, cwd_rel: &str) -> Value {
    let cwd = confine(cwd_rel).unwrap_or_else(fs_root);
    // handle builtin cd for cwd tracking
    let trimmed = cmd.trim();
    if let Some(arg) = trimmed.strip_prefix("cd").map(|s| s.trim()) {
        if trimmed == "cd" || arg.is_empty() {
            return json!({ "out": "", "cwd": rel_of(&fs_root()) });
        }
        let target = if arg.starts_with('/') {
            arg.to_string()
        } else {
            format!("{}/{}", rel_of(&cwd), arg)
        };
        return match confine(&target) {
            Some(p) if p.is_dir() => json!({ "out": "", "cwd": rel_of(&p) }),
            _ => {
                json!({ "out": format!("cd: {arg}: no such directory (confined to workspace)"), "cwd": rel_of(&cwd) })
            }
        };
    }
    let out = Command::new("/bin/sh")
        .arg("-lc")
        .arg(cmd)
        .current_dir(&cwd)
        .env("HOME", fs_root())
        .output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            json!({ "out": s, "cwd": rel_of(&cwd), "code": o.status.code().unwrap_or(-1) })
        }
        Err(e) => json!({ "out": format!("exec error: {e}"), "cwd": rel_of(&cwd) }),
    }
}

// ---------------- Power (guarded) ----------------
pub fn power(action: &str) -> Result<Value, String> {
    let arg = match action {
        "poweroff" => "poweroff",
        "reboot" => "reboot",
        "suspend" => "suspend",
        _ => return Err("unknown action".into()),
    };
    // Prefer systemctl; requires privilege. Report the attempt honestly.
    let out = Command::new("systemctl")
        .arg(arg)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({ "ok": true, "action": arg }))
    } else {
        Err(format!(
            "{} requires privilege: {}",
            arg,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static ENVLOCK: Mutex<()> = Mutex::new(());
    #[test]
    fn confine_blocks_traversal() {
        let _g = ENVLOCK.lock().unwrap();
        std::env::set_var("FABRIC_ROOT", "/tmp/fabric-test-confine");
        assert!(confine("/Documents/a.txt").is_some());
        assert!(confine("/").is_some());
        assert!(confine("/../../etc/passwd").is_none());
        assert!(confine("/Documents/../../../etc/shadow").is_none());
    }
    #[test]
    fn fs_crud_roundtrip() {
        let _g = ENVLOCK.lock().unwrap();
        let dir = format!("/tmp/fabric-test-{}", std::process::id());
        std::env::set_var("FABRIC_ROOT", &dir);
        let _ = std::fs::remove_dir_all(&dir);
        seed_root();
        fs_mkdir("/Documents/t").unwrap();
        fs_write("/Documents/t/f.txt", "hello").unwrap();
        assert_eq!(fs_read("/Documents/t/f.txt").unwrap()["content"], "hello");
        fs_trash("/Documents/t/f.txt").unwrap();
        assert!(fs_read("/Documents/t/f.txt").is_err());
        let t = trash_list().unwrap();
        let id = t["items"][0]["id"].as_str().unwrap().to_string();
        trash_restore(&id).unwrap();
        assert!(fs_read("/Documents/t/f.txt").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn real_processes_and_monitor() {
        assert!(!ps_list()["processes"].as_array().unwrap().is_empty());
        let m = monitor();
        assert!(m["mem_total"].as_u64().unwrap() > 0);
    }
}

// ---------------- Battery / power (real read) ----------------
pub fn battery() -> Value {
    let ps = std::path::Path::new("/sys/class/power_supply");
    let mut bat = json!(null);
    let mut ac = false;
    if let Ok(rd) = std::fs::read_dir(ps) {
        for e in rd.flatten() {
            let ty = std::fs::read_to_string(e.path().join("type")).unwrap_or_default();
            let ty = ty.trim();
            if ty == "Battery" {
                let cap = std::fs::read_to_string(e.path().join("capacity"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok());
                let st = std::fs::read_to_string(e.path().join("status"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                bat = json!({ "capacity": cap, "status": st, "name": e.file_name().to_string_lossy() });
            } else if (ty == "Mains" || ty == "USB")
                && std::fs::read_to_string(e.path().join("online"))
                    .unwrap_or_default()
                    .trim()
                    == "1"
            {
                ac = true;
            }
        }
    }
    json!({ "battery": bat, "ac": ac })
}

// ---------------- Date / time (real) ----------------
pub fn datetime() -> Value {
    let out = Command::new("timedatectl").arg("show").output().ok();
    let mut map = serde_json::Map::new();
    if let Some(o) = out {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some((k, v)) = l.split_once('=') {
                map.insert(k.to_string(), json!(v));
            }
        }
    }
    json!({ "timezone": map.get("Timezone").cloned().unwrap_or(json!("")),
            "ntp": map.get("NTP").map(|v| v.as_str()==Some("yes")).unwrap_or(false),
            "local": map.get("TimeUSec").cloned().unwrap_or(json!("")) })
}
pub fn set_ntp(on: bool) -> Result<Value, String> {
    let out = Command::new("timedatectl")
        .args(["set-ntp", if on { "true" } else { "false" }])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({"ok":true}))
    } else {
        Err(format!(
            "privilege required: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
pub fn set_time(iso: &str) -> Result<Value, String> {
    let out = Command::new("timedatectl")
        .args(["set-time", iso])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({"ok":true}))
    } else {
        Err(format!(
            "privilege required: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

// ---------------- Displays (real read) ----------------
pub fn displays() -> Value {
    // Try DRM connectors first (works headless-ish), then xrandr.
    let mut list = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/class/drm") {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !n.contains('-') {
                continue;
            }
            let status = std::fs::read_to_string(e.path().join("status"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if status.is_empty() {
                continue;
            }
            let modes = std::fs::read_to_string(e.path().join("modes")).unwrap_or_default();
            let res = modes.lines().next().unwrap_or("").to_string();
            list.push(json!({ "name": n.split_once('-').map(|x| x.1).unwrap_or(&n), "connected": status=="connected", "resolution": res }));
        }
    }
    json!({ "displays": list })
}

// ---------------- Wi-Fi (real via nmcli) ----------------
pub fn wifi_list() -> Value {
    let out = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "dev",
            "wifi",
            "list",
        ])
        .output()
        .ok();
    let mut nets = Vec::new();
    let mut have = true;
    match out {
        Some(o) if o.status.success() => {
            for l in String::from_utf8_lossy(&o.stdout).lines() {
                let c: Vec<&str> = l.split(':').collect();
                if c.len() < 4 || c[1].is_empty() {
                    continue;
                }
                nets.push(json!({ "active": c[0]=="*", "ssid": c[1], "signal": c[2].parse::<i64>().unwrap_or(0), "security": c[3] }));
            }
        }
        _ => have = false,
    }
    json!({ "networks": nets, "available": have })
}
pub fn wifi_connect(ssid: &str, pass: &str) -> Result<Value, String> {
    let mut args = vec!["dev", "wifi", "connect", ssid];
    if !pass.is_empty() {
        args.push("password");
        args.push(pass);
    }
    let out = Command::new("nmcli")
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({"ok":true}))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
pub fn wifi_disconnect(ssid: &str) -> Result<Value, String> {
    let out = Command::new("nmcli")
        .args(["con", "down", ssid])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({"ok":true}))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------- Email send (real via curl SMTP) ----------------
#[allow(clippy::too_many_arguments)]
pub fn send_mail(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<Value, String> {
    if host.is_empty() || user.is_empty() {
        return Err("mail not configured (Settings → Mail)".into());
    }
    let msg = format!("From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n");
    let url = format!("smtps://{host}:{port}");
    let mut child = Command::new("curl")
        .args([
            "-sS",
            "--ssl-reqd",
            &url,
            "--mail-from",
            from,
            "--mail-rcpt",
            to,
            "-T",
            "-",
            "-u",
            &format!("{user}:{pass}"),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    use std::io::Write as _;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(msg.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({ "ok": true, "to": to }))
    } else {
        Err(format!(
            "send failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

// ---------------- Cloud AI (real via curl, OpenAI-compatible) ----------------
pub fn cloud_chat(endpoint: &str, model: &str, key: &str, prompt: &str) -> Result<String, String> {
    if endpoint.is_empty() || key.is_empty() {
        return Err("cloud provider not configured".into());
    }
    let body = json!({ "model": model, "messages": [{"role":"user","content":prompt}], "max_tokens": 300 }).to_string();
    use std::io::Write as _;
    let mut child = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "60",
            "-X",
            "POST",
            endpoint,
            "-H",
            "content-type: application/json",
            "-H",
            &format!("authorization: Bearer {key}"),
            "--data-binary",
            "@-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("cloud request failed (network)".into());
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(String::from)
        .or_else(|| v["content"][0]["text"].as_str().map(String::from))
        .ok_or_else(|| {
            format!(
                "unexpected response: {}",
                String::from_utf8_lossy(&out.stdout)
                    .chars()
                    .take(120)
                    .collect::<String>()
            )
        })
}

// ---------------- Applications (real .desktop apps) ----------------
pub fn apps_installed() -> Value {
    let mut apps = Vec::new();
    let dirs = [
        "/usr/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
            std::env::var("HOME").unwrap_or_default()
        ),
        "/var/lib/flatpak/exports/share/applications".to_string(),
    ];
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                if e.path().extension().and_then(|x| x.to_str()) != Some("desktop") {
                    continue;
                }
                let txt = std::fs::read_to_string(e.path()).unwrap_or_default();
                let get = |k: &str| {
                    txt.lines()
                        .find(|l| l.starts_with(k))
                        .and_then(|l| l.split_once('='))
                        .map(|(_, v)| v.trim().to_string())
                        .unwrap_or_default()
                };
                if get("NoDisplay") == "true" || get("Type") != "Application" {
                    continue;
                }
                let name = get("Name");
                let exec = get("Exec");
                if name.is_empty() || exec.is_empty() {
                    continue;
                }
                apps.push(
                    json!({ "name": name, "exec": exec, "comment": get("Comment"),
                    "categories": get("Categories"), "file": e.file_name().to_string_lossy() }),
                );
            }
        }
    }
    apps.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
    });
    apps.dedup_by(|a, b| a["name"] == b["name"]);
    json!({ "apps": apps })
}
/// Launch a .desktop Exec line on the host display (real). Detached.
pub fn app_launch(exec: &str) -> Result<Value, String> {
    // strip desktop field codes
    let cleaned: String = exec
        .split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err("empty command".into());
    }
    Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("setsid {cleaned} >/dev/null 2>&1 &"))
        .env(
            "DISPLAY",
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()),
        )
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "launched": cleaned }))
}
/// Install an apt package (privileged; real). Guarded by confirmation upstream.
pub fn pkg_mgr() -> &'static str {
    if std::path::Path::new("/usr/bin/apt-get").exists() {
        "apt"
    } else if std::path::Path::new("/usr/bin/dnf").exists() {
        "dnf"
    } else if std::path::Path::new("/usr/bin/pacman").exists() {
        "pacman"
    } else {
        "none"
    }
}
pub fn app_install(pkg: &str) -> Result<Value, String> {
    if pkg.is_empty()
        || !pkg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-+._".contains(c))
    {
        return Err("invalid package name".into());
    }
    let out = match pkg_mgr() {
        "apt" => Command::new("apt-get")
            .args(["install", "-y", pkg])
            .output(),
        "dnf" => Command::new("dnf").args(["install", "-y", pkg]).output(),
        "pacman" => Command::new("pacman")
            .args(["-S", "--noconfirm", pkg])
            .output(),
        _ => return Err("no supported package manager".into()),
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({ "ok": true, "installed": pkg }))
    } else {
        Err(format!(
            "install needs privilege or package not found: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
        ))
    }
}
pub fn app_search(q: &str) -> Value {
    if q.is_empty() {
        return json!({ "results": [], "available": true, "mgr": pkg_mgr() });
    }
    let mut res = Vec::new();
    match pkg_mgr() {
        "apt" => {
            if let Ok(o) = Command::new("apt-cache").args(["search", q]).output() {
                for l in String::from_utf8_lossy(&o.stdout).lines().take(40) {
                    if let Some((n, d)) = l.split_once(" - ") {
                        res.push(json!({ "pkg": n.trim(), "desc": d.trim() }));
                    }
                }
            }
        }
        "dnf" => {
            if let Ok(o) = Command::new("dnf").args(["-q", "search", q]).output() {
                for l in String::from_utf8_lossy(&o.stdout).lines() {
                    if let Some((n, d)) = l.split_once(" : ") {
                        let name = n.trim().rsplit_once('.').map(|x| x.0).unwrap_or(n.trim());
                        res.push(json!({ "pkg": name, "desc": d.trim() }));
                    }
                }
                res.truncate(40);
            }
        }
        "pacman" => {
            if let Ok(o) = Command::new("pacman").args(["-Ss", q]).output() {
                let t = String::from_utf8_lossy(&o.stdout);
                let mut itr = t.lines();
                while let Some(h) = itr.next() {
                    if let Some(d) = itr.next() {
                        let name = h
                            .split('/')
                            .nth(1)
                            .and_then(|x| x.split_whitespace().next())
                            .unwrap_or("");
                        if !name.is_empty() {
                            res.push(json!({ "pkg": name, "desc": d.trim() }));
                        }
                    }
                }
                res.truncate(40);
            }
        }
        _ => {}
    }
    json!({ "results": res, "available": !res.is_empty(), "mgr": pkg_mgr() })
}
pub fn updates_check() -> Value {
    let mut ups = Vec::new();
    match pkg_mgr() {
        "apt" => {
            let _ = Command::new("apt-get").args(["update", "-qq"]).output();
            if let Ok(o) = Command::new("apt").args(["list", "--upgradable"]).output() {
                for l in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
                    if let Some((n, _)) = l.split_once('/') {
                        ups.push(json!({ "pkg": n }));
                    }
                }
            }
        }
        "dnf" => {
            if let Ok(o) = Command::new("dnf").args(["-q", "check-update"]).output() {
                for l in String::from_utf8_lossy(&o.stdout).lines() {
                    let c: Vec<&str> = l.split_whitespace().collect();
                    if c.len() >= 3 && c[0].contains('.') {
                        ups.push(json!({ "pkg": c[0], "version": c[1] }));
                    }
                }
            }
        }
        _ => {}
    }
    let n = ups.len();
    json!({ "updates": ups, "count": n, "mgr": pkg_mgr() })
}
pub fn updates_apply() -> Result<Value, String> {
    let out = match pkg_mgr() {
        "apt" => Command::new("apt-get").args(["upgrade", "-y"]).output(),
        "dnf" => Command::new("dnf").args(["upgrade", "-y"]).output(),
        "pacman" => Command::new("pacman")
            .args(["-Syu", "--noconfirm"])
            .output(),
        _ => return Err("no package manager".into()),
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(json!({ "ok": true }))
    } else {
        Err(format!(
            "update needs privilege: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
        ))
    }
}

// ---------------- Fabric OS self-update (GitHub releases) ----------------
pub const OS_VERSION: &str = "0.2.0-fabric";
pub fn os_update_check() -> Value {
    let url = "https://api.github.com/repos/PatienceAIiN/fabric-os/releases/latest";
    let out = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: FabricOS",
            url,
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            if let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) {
                let latest = v["tag_name"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches('v')
                    .to_string();
                let newer = !latest.is_empty() && latest != OS_VERSION;
                return json!({ "current": OS_VERSION, "latest": latest, "newer": newer,
                    "url": v["html_url"], "notes": v["body"], "available": true });
            }
            json!({ "current": OS_VERSION, "available": false, "error": "bad response" })
        }
        _ => {
            json!({ "current": OS_VERSION, "available": false, "error": "offline or repo unreachable" })
        }
    }
}

// ---------------- Autonomous agent (server-side: plan -> execute) ----------------
fn cap_first(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
fn categorize(name: &str) -> &'static str {
    let e = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match e.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "Images",
        "pdf" | "txt" | "doc" | "docx" | "md" | "odt" | "rtf" => "Documents",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "Music",
        "mp4" | "mkv" | "webm" | "avi" | "mov" => "Videos",
        "zip" | "tar" | "gz" | "xz" | "7z" | "bz2" => "Archives",
        _ => "Other",
    }
}
/// Build a plan (list of {tool,args,why}) for a natural-language goal.
pub fn agent_plan(goal: &str) -> Vec<Value> {
    let g = goal.to_lowercase();
    let mut plan = Vec::new();
    // organize / clean up / arrange <folder>
    if g.contains("organi") || g.contains("clean") || g.contains("arrange") || g.contains("sort") {
        let folder = [
            "downloads",
            "documents",
            "pictures",
            "music",
            "videos",
            "desktop",
        ]
        .iter()
        .find(|f| g.contains(*f))
        .map(|f| format!("/{}", cap_first(f)))
        .unwrap_or_else(|| "/Downloads".into());
        if let Ok(listing) = fs_list(&folder) {
            let mut seen = std::collections::HashSet::new();
            for it in listing["items"].as_array().cloned().unwrap_or_default() {
                if it["dir"].as_bool().unwrap_or(false) {
                    continue;
                }
                let name = it["name"].as_str().unwrap_or("");
                let c = categorize(name);
                if seen.insert(c) {
                    plan.push(json!({"tool":"mkdir","args":{"path":format!("{folder}/{c}")},"why":format!("folder for {c}")}));
                }
                plan.push(json!({"tool":"move","args":{"path":it["path"],"to":format!("{folder}/{c}")},"why":format!("move {name}")}));
            }
            if !plan.is_empty() {
                return plan;
            }
        }
    }
    // create/make folder or project <name>
    if let Some(pos) = ["create", "make", "new "].iter().find_map(|k| g.find(k)) {
        let _ = pos;
        // extract name after folder/project
        let after = g.rsplit(' ').collect::<Vec<_>>();
        let _ = after;
        if let Some(idx) = g
            .find("folder")
            .or_else(|| g.find("project"))
            .or_else(|| g.find("directory"))
        {
            let raw = &goal[idx..];
            let name: String = raw
                .split_once(' ')
                .map(|x| x.1)
                .unwrap_or("New Folder")
                .replace("called ", "")
                .replace("named ", "")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
                .collect::<String>()
                .trim()
                .to_string();
            let name = if name.is_empty() {
                "New Folder".into()
            } else {
                name
            };
            let base = format!("/Documents/{name}");
            if g.contains("project") {
                plan.push(json!({"tool":"mkdir","args":{"path":base},"why":"project root"}));
                plan.push(json!({"tool":"mkdir","args":{"path":format!("{base}/src")},"why":"source dir"}));
                plan.push(
                    json!({"tool":"mkdir","args":{"path":format!("{base}/docs")},"why":"docs dir"}),
                );
                plan.push(json!({"tool":"write","args":{"path":format!("{base}/README.md"),"content":format!("# {name}\n\nCreated autonomously by the Fabric OS agent.\n")},"why":"README"}));
            } else {
                plan.push(json!({"tool":"mkdir","args":{"path":base},"why":"create folder"}));
            }
            return plan;
        }
    }
    // write a note / file named X saying Y
    if g.contains("note") || g.contains("write") {
        let content = goal
            .split_once("saying")
            .map(|x| x.1)
            .or_else(|| goal.split_once("with").map(|x| x.1))
            .unwrap_or("Hello from Fabric OS.")
            .trim()
            .to_string();
        plan.push(json!({"tool":"write","args":{"path":"/Documents/note.txt","content":content},"why":"write note"}));
        return plan;
    }
    // run / execute a shell command (real)
    if g.starts_with("run ") || g.starts_with("execute ") || g.contains("command:") {
        let cmd = goal.splitn(2, ' ').nth(1).unwrap_or("").to_string();
        let cmd = cmd.trim_start_matches("command:").trim().to_string();
        if !cmd.is_empty() {
            plan.push(json!({"tool":"exec","args":{"cmd":cmd,"cwd":"/"},"why":"run command"}));
            return plan;
        }
    }
    // find / search X
    if let Some(k) = ["find ", "search "].iter().find(|k| g.contains(**k)) {
        let q = goal
            .split_once(k.trim())
            .map(|x| x.1)
            .unwrap_or("")
            .trim()
            .trim_start_matches("for ")
            .trim_start_matches("all ")
            .trim_start_matches("*.")
            .to_string();
        plan.push(json!({"tool":"search","args":{"path":"/","q":q},"why":"search workspace"}));
        return plan;
    }
    plan
}
fn agent_exec_tool(tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "mkdir" => fs_mkdir(args["path"].as_str().unwrap_or("")),
        "write" => fs_write(
            args["path"].as_str().unwrap_or(""),
            args["content"].as_str().unwrap_or(""),
        ),
        "move" => fs_move(
            args["path"].as_str().unwrap_or(""),
            args["to"].as_str().unwrap_or(""),
        ),
        "trash" => fs_trash(args["path"].as_str().unwrap_or("")),
        "search" => fs_search("/", args["q"].as_str().unwrap_or("")),
        "list" => fs_list(args["path"].as_str().unwrap_or("/")),
        "exec" => {
            let r = exec(
                args["cmd"].as_str().unwrap_or(""),
                args["cwd"].as_str().unwrap_or("/"),
            );
            let code = r["code"].as_i64().unwrap_or(0);
            if code == 0 {
                Ok(r)
            } else {
                Err(format!(
                    "exit {code}: {}",
                    r["out"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(200)
                        .collect::<String>()
                ))
            }
        }
        "notify" => Ok(json!({"ok":true})),
        _ => Err(format!("unknown tool {tool}")),
    }
}
/// Plan + autonomously execute. `serious` tools (move/trash) run only if
/// `autonomous`, else they are returned as awaiting-approval.
pub fn agent_run(goal: &str, autonomous: bool) -> Value {
    let plan = agent_plan(goal);
    if plan.is_empty() {
        return json!({ "steps": [], "done": 0, "skipped": 0, "failed": 0, "total": 0,
            "message": "No actionable plan. Try: 'organize my downloads', 'create a project called X', 'find <name>'." });
    }
    let serious = ["move", "trash", "launch", "exec"];
    let (mut done, mut skipped, mut failed) = (0, 0, 0);
    let mut steps = Vec::new();
    for st in &plan {
        let tool = st["tool"].as_str().unwrap_or("");
        let args = &st["args"];
        if serious.contains(&tool) && !autonomous {
            steps.push(json!({ "tool": tool, "args": args, "why": st["why"], "status": "awaiting-approval" }));
            skipped += 1;
            continue;
        }
        match agent_exec_tool(tool, args) {
            Ok(r) => {
                done += 1;
                steps.push(json!({ "tool": tool, "args": args, "why": st["why"], "status": "done", "result": r }));
            }
            Err(e) => {
                failed += 1;
                steps.push(json!({ "tool": tool, "args": args, "why": st["why"], "status": "failed", "error": e }));
            }
        }
    }
    json!({ "steps": steps, "done": done, "skipped": skipped, "failed": failed, "total": plan.len(),
        "message": format!("Task complete: {done} done, {skipped} awaiting approval, {failed} failed") })
}
