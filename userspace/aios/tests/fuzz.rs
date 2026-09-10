//! Parser/robustness fuzzing (spec §80 "fuzz parsers"). Security-critical
//! parsers must never panic on arbitrary input, and confinement/authorization
//! invariants must hold for every generated case. Deterministic PRNG (no deps),
//! so failures are reproducible.
use libcapability::{normalize_abs_path, Capability, CapabilitySet};
use libprovenance::{redact, PolicyDecision, ProvenanceLog};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 33) as u8
    }
    fn string(&mut self, max: usize) -> String {
        let n = (self.next() as usize) % (max + 1);
        // bias toward capability-ish characters to hit parser branches
        const ALPH: &[u8] = b"fsnetmodelgpu.:/-_abcABC0123 \t\n\x00\xff";
        (0..n)
            .map(|_| ALPH[(self.byte() as usize) % ALPH.len()] as char)
            .collect()
    }
}

#[test]
fn fuzz_capability_parse_never_panics() {
    let mut rng = Lcg(0xDEADBEEF);
    for _ in 0..200_000 {
        let s = rng.string(40);
        // must return Ok or Err, never panic/hang
        let _ = Capability::parse(&s);
    }
}

#[test]
fn fuzz_normalize_path_never_escapes_root() {
    let mut rng = Lcg(0x12345);
    for _ in 0..200_000 {
        let s = format!("/{}", rng.string(48));
        if let Some(norm) = normalize_abs_path(&s) {
            // invariant: a normalized absolute path never contains a ".." and
            // never leaves the root.
            assert!(norm.starts_with('/'));
            assert!(!norm.split('/').any(|c| c == ".."));
        }
    }
}

#[test]
fn fuzz_capabilityset_permits_confinement_invariant() {
    // A set granting only /data must NEVER permit a read outside /data,
    // whatever weird request string is thrown at it.
    let set = CapabilitySet::parse_all(["fs.read:/data"]).unwrap();
    let mut rng = Lcg(0xABCDEF);
    for _ in 0..200_000 {
        let req = format!("fs.read:/{}", rng.string(48));
        if set.permits_str(&req) {
            // if permitted, the normalized path must be within /data
            let path = req.trim_start_matches("fs.read:");
            let norm = normalize_abs_path(path).expect("permitted => parseable");
            assert!(
                norm == "/data" || norm.starts_with("/data/"),
                "leak: permitted {req} -> {norm}"
            );
        }
    }
}

#[test]
fn fuzz_redact_and_provenance_robust() {
    let mut rng = Lcg(0x55AA);
    let mut log = ProvenanceLog::new();
    for i in 0..5_000 {
        let junk = rng.string(64);
        let _ = redact(&junk); // never panics
        log.append(
            i,
            "actor",
            "agent",
            "intent",
            "model",
            "tool",
            "action",
            &junk,
            "result",
            PolicyDecision::Allowed,
            None,
        );
    }
    // chain still verifies after thousands of arbitrary-content records
    assert!(log.verify().is_ok());
}
