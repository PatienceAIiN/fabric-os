//! AI resource fabric (M20).
//!
//! Represents compute resources without hiding hardware detail, accounts
//! reservations, and translates an agent budget into the same cgroup-scope
//! enforcement `tools/rg` uses — so resource exhaustion cannot destabilise the
//! system (threat T-003).
#![forbid(unsafe_code)]

use libagent::ResourceLimits;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Cpu,
    Gpu,
    Npu,
    Ram,
    Vram,
    Storage,
    Network,
    Energy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    pub kind: ResourceKind,
    pub name: String,
    pub capacity: u64,
    pub available: u64,
    pub unit: String,
}

/// Probe the host for real resources (best-effort, Linux /proc + /sys).
pub fn probe_host() -> Vec<Resource> {
    let mut out = Vec::new();
    // CPU: count processors.
    if let Ok(txt) = std::fs::read_to_string("/proc/cpuinfo") {
        let cpus = txt.lines().filter(|l| l.starts_with("processor")).count() as u64;
        out.push(Resource {
            kind: ResourceKind::Cpu,
            name: "cpu".into(),
            capacity: cpus * 100, // percent-cores
            available: cpus * 100,
            unit: "percent-cores".into(),
        });
    }
    // RAM: MemTotal / MemAvailable.
    if let Ok(txt) = std::fs::read_to_string("/proc/meminfo") {
        let get = |k: &str| {
            txt.lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
                .unwrap_or(0)
        };
        out.push(Resource {
            kind: ResourceKind::Ram,
            name: "ram".into(),
            capacity: get("MemTotal:"),
            available: get("MemAvailable:"),
            unit: "bytes".into(),
        });
    }
    // GPU: detect DRM render nodes.
    if let Ok(rd) = std::fs::read_dir("/sys/class/drm") {
        let cards = rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("card"))
            .filter(|e| !e.file_name().to_string_lossy().contains('-'))
            .count() as u64;
        if cards > 0 {
            out.push(Resource {
                kind: ResourceKind::Gpu,
                name: "gpu".into(),
                capacity: cards * 100,
                available: cards * 100,
                unit: "percent".into(),
            });
        }
    }
    out
}

/// Translate an agent budget into systemd-run scope properties (the exact
/// mechanism tools/rg uses). Returned as CLI args for `systemd-run --user
/// --scope`.
pub fn budget_to_scope_args(b: &ResourceLimits) -> Vec<String> {
    let mut v = vec![
        "--scope".to_string(),
        format!("--property=MemoryMax={}", b.ram_bytes),
        format!(
            "--property=MemoryHigh={}",
            (b.ram_bytes as f64 * 0.85) as u64
        ),
        format!("--property=CPUQuota={}%", b.cpu_percent),
    ];
    if b.duration_secs > 0 {
        v.push(format!("--property=RuntimeMaxSec={}", b.duration_secs));
    }
    v
}

/// A simple reservation ledger to prevent overcommit of a resource pool.
#[derive(Default)]
pub struct Ledger {
    reserved: HashMap<String, u64>,
    capacity: HashMap<String, u64>,
}

impl Ledger {
    pub fn new() -> Self {
        Ledger::default()
    }
    pub fn set_capacity(&mut self, name: &str, cap: u64) {
        self.capacity.insert(name.to_string(), cap);
    }
    pub fn reserve(&mut self, name: &str, amount: u64) -> Result<(), String> {
        let cap = *self.capacity.get(name).unwrap_or(&0);
        let cur = *self.reserved.get(name).unwrap_or(&0);
        if cur + amount > cap {
            return Err(format!(
                "overcommit: {name} needs {amount}, only {} free",
                cap.saturating_sub(cur)
            ));
        }
        *self.reserved.entry(name.to_string()).or_insert(0) += amount;
        Ok(())
    }
    pub fn release(&mut self, name: &str, amount: u64) {
        if let Some(r) = self.reserved.get_mut(name) {
            *r = r.saturating_sub(amount);
        }
    }
    pub fn reserved(&self, name: &str) -> u64 {
        *self.reserved.get(name).unwrap_or(&0)
    }
}

/// A compute accelerator choice for a workload (M23 fabric routing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accelerator {
    Cpu,
    Gpu,
    Npu,
}

/// Detect accelerators actually present on this host.
pub fn detect_accelerators() -> Vec<Accelerator> {
    let mut v = vec![Accelerator::Cpu];
    // GPU: a DRM render node.
    if std::path::Path::new("/dev/dri/renderD128").exists() {
        v.push(Accelerator::Gpu);
    }
    // NPU: Linux accel subsystem (/dev/accel*) — none on typical laptops.
    if let Ok(rd) = std::fs::read_dir("/dev") {
        if rd
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("accel"))
        {
            v.push(Accelerator::Npu);
        }
    }
    v
}

/// Select an accelerator for a workload by preference + availability + memory
/// (M13/M77 routing). `prefer_low_power` biases toward NPU>GPU>CPU when present.
pub fn select_accelerator(
    available: &[Accelerator],
    required_mem: u64,
    gpu_free_mem: u64,
    prefer_low_power: bool,
) -> (Accelerator, String) {
    let has = |a: Accelerator| available.contains(&a);
    if prefer_low_power && has(Accelerator::Npu) {
        return (Accelerator::Npu, "NPU: lowest power".into());
    }
    if has(Accelerator::Gpu) {
        if required_mem <= gpu_free_mem {
            return (Accelerator::Gpu, "GPU: fits in VRAM, lower latency".into());
        }
        return (
            Accelerator::Cpu,
            "GPU present but model exceeds free VRAM".into(),
        );
    }
    (Accelerator::Cpu, "CPU: no accelerator available".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerator_detection_and_selection() {
        let acc = detect_accelerators();
        assert!(acc.contains(&Accelerator::Cpu));
        // this host has a GPU render node
        assert!(acc.contains(&Accelerator::Gpu));
        // GPU chosen when model fits VRAM
        let (a, _) = select_accelerator(&acc, 500_000_000, 1_800_000_000, false);
        assert_eq!(a, Accelerator::Gpu);
        // falls back to CPU when model too big for VRAM
        let (b, _) = select_accelerator(&acc, 8_000_000_000, 1_800_000_000, false);
        assert_eq!(b, Accelerator::Cpu);
        // CPU-only host
        let (c, _) = select_accelerator(&[Accelerator::Cpu], 1, 0, false);
        assert_eq!(c, Accelerator::Cpu);
    }

    #[test]
    fn probes_cpu_and_ram_on_this_host() {
        let r = probe_host();
        assert!(r
            .iter()
            .any(|x| x.kind == ResourceKind::Cpu && x.capacity > 0));
        assert!(r
            .iter()
            .any(|x| x.kind == ResourceKind::Ram && x.capacity > 0));
    }

    #[test]
    fn budget_maps_to_cgroup_scope() {
        let b = ResourceLimits {
            cpu_percent: 200,
            ram_bytes: 2 * 1024 * 1024 * 1024,
            gpu_percent: 0,
            net_bps: 0,
            duration_secs: 60,
        };
        let args = budget_to_scope_args(&b);
        assert!(args.iter().any(|a| a.contains("MemoryMax=2147483648")));
        assert!(args.iter().any(|a| a.contains("CPUQuota=200%")));
        assert!(args.iter().any(|a| a.contains("RuntimeMaxSec=60")));
    }

    #[test]
    fn ledger_prevents_overcommit() {
        let mut l = Ledger::new();
        l.set_capacity("ram", 1000);
        l.reserve("ram", 600).unwrap();
        assert!(l.reserve("ram", 500).is_err());
        l.reserve("ram", 400).unwrap();
        assert_eq!(l.reserved("ram"), 1000);
        l.release("ram", 1000);
        assert_eq!(l.reserved("ram"), 0);
    }
}
