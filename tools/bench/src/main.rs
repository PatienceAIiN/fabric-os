//! Microbenchmarks for the deterministic core. Prints ns/op; makes no claim
//! beyond what it measures on the current host. Run: `aios-bench [iters]`.
#![forbid(unsafe_code)]

use libagent::{Agent, ResourceLimits, TrustLevel};
use libcapability::{Capability, CapabilitySet};
use libgovernor::evaluate;
use libintent::{ApprovalPolicy, Intent, RiskLevel};
use libprovenance::ProvenanceLog;
use std::hint::black_box;
use std::time::Instant;

fn bench<F: FnMut()>(name: &str, iters: u64, mut f: F) {
    // warmup
    for _ in 0..(iters / 10).max(1) {
        f();
    }
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    let per_s = 1e9 / ns;
    println!("  {:<34} {:>10.1} ns/op   {:>12.0} ops/s", name, ns, per_s);
}

fn main() {
    let iters: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    println!("ai-native-os microbenchmarks  (iters={iters}, host-measured)");

    let set = CapabilitySet::parse_all(["fs.read:/home/harsh/Downloads"]).unwrap();
    let req = Capability::parse("fs.read:/home/harsh/Downloads/a.txt").unwrap();
    bench("capability permits()", iters, || {
        black_box(set.permits(black_box(&req)));
    });

    bench("sha256 (64B)", iters, || {
        black_box(libcrypto::sha256(black_box(
            b"the quick brown fox jumps over the lazy dog!!64B",
        )));
    });

    let agent = Agent::create(
        "u",
        "p",
        "local/reasoning",
        TrustLevel::Limited,
        ResourceLimits::default(),
        1_000_000_000,
    );
    let mut intent = Intent::create(
        "u",
        "obj",
        "scope",
        vec!["fs.read:/home/harsh/Downloads".into()],
        RiskLevel::Medium,
        ApprovalPolicy::AtOrAbove(RiskLevel::High),
        ResourceLimits::default(),
        0,
        1_000_000_000,
    );
    intent.validate().unwrap();
    intent.authorize().unwrap();
    intent.begin_execution().unwrap();

    let cached_set = intent.capability_set().unwrap();
    let mut sink0 = ProvenanceLog::new();
    bench("governor evaluate_with_caps()", iters, || {
        black_box(libgovernor::evaluate_with_caps(
            &agent,
            &intent,
            &cached_set,
            "fs.read:/home/harsh/Downloads/x",
            None,
            1,
            &mut sink0,
        ));
    });
    let mut sink = ProvenanceLog::new();
    bench("governor evaluate() (+parse+prov)", iters, || {
        black_box(evaluate(
            &agent,
            &intent,
            "fs.read:/home/harsh/Downloads/x",
            None,
            1,
            &mut sink,
        ));
    });

    let mut plog = ProvenanceLog::new();
    bench("provenance append()", iters, || {
        plog.append(
            1,
            "u",
            "a",
            "i",
            "m",
            "t",
            "act",
            "res",
            "ok",
            libprovenance::PolicyDecision::Allowed,
            None,
        );
    });
    let t = Instant::now();
    let ok = plog.verify().is_ok();
    let vns = t.elapsed().as_nanos() as f64 / plog.len().max(1) as f64;
    println!(
        "  {:<34} {:>10.1} ns/record  (chain valid: {})",
        "provenance verify()", vns, ok
    );
    println!("done.");
}
