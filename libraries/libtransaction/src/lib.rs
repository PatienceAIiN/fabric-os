//! Reversible execution for filesystem operations.
//!
//! A [`Transaction`] plans operations, previews their effect and reversibility,
//! executes them inside a confinement root, and can roll back supported
//! changes. Deletes are never true unlinks: files are moved to a per-
//! transaction backup area, making delete *compensatable* (restorable). We do
//! not pretend external side effects are reversible; each op is classified.
//!
//! All paths are canonicalized lexically and confined to a root; model-
//! supplied paths that escape the root are rejected (never trust them).
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reversibility {
    Reversible,
    Compensatable,
    Irreversible,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    /// Move/rename a file or dir within the root.
    Move { from: String, to: String },
    /// Delete a file (implemented as move-to-backup => compensatable).
    Delete { path: String },
    /// Create a directory.
    Mkdir { path: String },
}

impl Op {
    pub fn reversibility(&self) -> Reversibility {
        match self {
            Op::Move { .. } => Reversibility::Reversible,
            Op::Delete { .. } => Reversibility::Compensatable,
            Op::Mkdir { .. } => Reversibility::Reversible,
        }
    }
}

/// Lexically normalize and confine `p` under `root`. Returns the absolute,
/// normalized path if it stays within root, else None.
pub fn confine(root: &Path, p: &str) -> Option<PathBuf> {
    let joined = if p.starts_with('/') {
        PathBuf::from(p)
    } else {
        root.join(p)
    };
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for comp in joined.components() {
        use std::path::Component::*;
        match comp {
            Prefix(_) | RootDir => out.clear(),
            CurDir => {}
            ParentDir => {
                out.pop()?;
            }
            Normal(c) => out.push(c.to_os_string()),
        }
    }
    let mut abs = PathBuf::from("/");
    for c in &out {
        abs.push(c);
    }
    let rootn = normalize(root);
    if abs == rootn || abs.starts_with(&rootn) {
        Some(abs)
    } else {
        None
    }
}

fn normalize(p: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for comp in p.components() {
        use std::path::Component::*;
        match comp {
            Prefix(_) | RootDir => out.clear(),
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            Normal(c) => out.push(c.to_os_string()),
        }
    }
    let mut abs = PathBuf::from("/");
    for c in &out {
        abs.push(c);
    }
    abs
}

#[derive(Debug)]
enum Undo {
    MoveBack { from: PathBuf, to: PathBuf },
    RestoreFromBackup { backup: PathBuf, original: PathBuf },
    RemoveDir { path: PathBuf },
}

/// A transaction bound to a confinement root.
pub struct Transaction {
    root: PathBuf,
    backup: PathBuf,
    ops: Vec<Op>,
    undo: Vec<Undo>,
    committed: bool,
    backup_seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewLine {
    pub op: Op,
    pub reversibility: Reversibility,
    pub ok: bool,
    pub note: String,
}

impl Transaction {
    /// Begin a transaction; `backup` must also be inside/near root and is
    /// created on demand for compensatable deletes.
    pub fn begin(root: impl AsRef<Path>, backup: impl AsRef<Path>) -> Self {
        Transaction {
            root: normalize(root.as_ref()),
            backup: normalize(backup.as_ref()),
            ops: Vec::new(),
            undo: Vec::new(),
            committed: false,
            backup_seq: 0,
        }
    }

    pub fn add(&mut self, op: Op) {
        self.ops.push(op);
    }

    /// Preview: validate confinement and report reversibility without changing
    /// anything.
    pub fn preview(&self) -> Vec<PreviewLine> {
        self.ops
            .iter()
            .map(|op| {
                let (ok, note) = self.validate(op);
                PreviewLine {
                    op: op.clone(),
                    reversibility: op.reversibility(),
                    ok,
                    note,
                }
            })
            .collect()
    }

    fn validate(&self, op: &Op) -> (bool, String) {
        let check = |p: &str| confine(&self.root, p).is_some();
        match op {
            Op::Move { from, to } => {
                if !check(from) || !check(to) {
                    (false, "path escapes confinement root".into())
                } else {
                    (true, String::new())
                }
            }
            Op::Delete { path } | Op::Mkdir { path } => {
                if !check(path) {
                    (false, "path escapes confinement root".into())
                } else {
                    (true, String::new())
                }
            }
        }
    }

    /// Execute all ops, recording undo information. On any failure, the caller
    /// should rollback. Fails closed on confinement violations.
    pub fn commit(&mut self) -> Result<(), String> {
        if self.committed {
            return Err("already committed".into());
        }
        for op in self.ops.clone() {
            self.exec(&op)?;
        }
        self.committed = true;
        Ok(())
    }

    fn exec(&mut self, op: &Op) -> Result<(), String> {
        match op {
            Op::Move { from, to } => {
                let f = confine(&self.root, from).ok_or("from escapes root")?;
                let t = confine(&self.root, to).ok_or("to escapes root")?;
                std::fs::rename(&f, &t).map_err(|e| format!("move {from}->{to}: {e}"))?;
                self.undo.push(Undo::MoveBack { from: t, to: f });
                Ok(())
            }
            Op::Mkdir { path } => {
                let p = confine(&self.root, path).ok_or("path escapes root")?;
                std::fs::create_dir_all(&p).map_err(|e| format!("mkdir {path}: {e}"))?;
                self.undo.push(Undo::RemoveDir { path: p });
                Ok(())
            }
            Op::Delete { path } => {
                let p = confine(&self.root, path).ok_or("path escapes root")?;
                std::fs::create_dir_all(&self.backup).map_err(|e| format!("backup dir: {e}"))?;
                self.backup_seq += 1;
                let b = self.backup.join(format!("{}.bak", self.backup_seq));
                std::fs::rename(&p, &b).map_err(|e| format!("delete(move) {path}: {e}"))?;
                self.undo.push(Undo::RestoreFromBackup {
                    backup: b,
                    original: p,
                });
                Ok(())
            }
        }
    }

    /// Roll back committed operations in reverse order (best-effort; returns
    /// the list of errors encountered).
    pub fn rollback(&mut self) -> Vec<String> {
        let mut errs = Vec::new();
        while let Some(u) = self.undo.pop() {
            let r = match u {
                Undo::MoveBack { from, to } => {
                    std::fs::rename(&from, &to).map_err(|e| format!("undo move: {e}"))
                }
                Undo::RestoreFromBackup { backup, original } => {
                    std::fs::rename(&backup, &original).map_err(|e| format!("undo delete: {e}"))
                }
                Undo::RemoveDir { path } => {
                    std::fs::remove_dir(&path).map_err(|e| format!("undo mkdir: {e}"))
                }
            };
            if let Err(e) = r {
                errs.push(e);
            }
        }
        self.committed = false;
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let uniq = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("aiostx-{}-{}", std::process::id(), uniq));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("root")).unwrap();
        d
    }

    #[test]
    fn confinement_rejects_escape() {
        let root = Path::new("/home/u/Downloads");
        assert!(confine(root, "a/b.txt").is_some());
        assert!(confine(root, "/home/u/Downloads/x").is_some());
        assert!(confine(root, "../../etc/passwd").is_none());
        assert!(confine(root, "/etc/passwd").is_none());
    }

    #[test]
    fn move_then_rollback_restores() {
        let d = tmp();
        let root = d.join("root");
        fs::write(root.join("a.txt"), b"hi").unwrap();
        let mut tx = Transaction::begin(&root, d.join("bak"));
        tx.add(Op::Move {
            from: "a.txt".into(),
            to: "b.txt".into(),
        });
        assert!(tx.preview().iter().all(|l| l.ok));
        tx.commit().unwrap();
        assert!(root.join("b.txt").exists() && !root.join("a.txt").exists());
        let errs = tx.rollback();
        assert!(errs.is_empty());
        assert!(root.join("a.txt").exists() && !root.join("b.txt").exists());
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn delete_is_compensatable_and_restores() {
        let d = tmp();
        let root = d.join("root");
        fs::write(root.join("junk"), b"x").unwrap();
        let mut tx = Transaction::begin(&root, d.join("bak"));
        tx.add(Op::Delete {
            path: "junk".into(),
        });
        assert_eq!(tx.preview()[0].reversibility, Reversibility::Compensatable);
        tx.commit().unwrap();
        assert!(!root.join("junk").exists());
        tx.rollback();
        assert!(root.join("junk").exists()); // restored from backup
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn escape_op_fails_closed_on_commit() {
        let d = tmp();
        let root = d.join("root");
        let mut tx = Transaction::begin(&root, d.join("bak"));
        tx.add(Op::Move {
            from: "../../etc/passwd".into(),
            to: "x".into(),
        });
        assert!(!tx.preview()[0].ok);
        assert!(tx.commit().is_err());
        fs::remove_dir_all(&d).ok();
    }
}
