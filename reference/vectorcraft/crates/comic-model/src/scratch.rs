//! A scratch directory for this crate's tests.
//!
//! It lives under the workspace's `target/`, not the system temporary directory, so a test run
//! never writes outside the checkout. Each name is unique per test, and the directory is removed
//! before use so a previous failure cannot be mistaken for a pass.
//!
//! The name is also **unique per process**, because two test runs can be in flight at once — a
//! reviewer's mutation harness runs `cargo test` on this crate while the owner runs it — and a
//! shared scratch path makes one run delete the directory the other is using, which shows up as a
//! spurious failure in whichever test loses the race. That is a false red, and a false red is
//! worse than no run: it costs a whole verification cycle to rule out.

use std::fs;
use std::path::{Path, PathBuf};

/// An empty directory for one test to write into.
pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = scratch_root().join(format!("{}-{name}", std::process::id()));
    fs::remove_dir_all(&dir).ok();
    dir
}

/// The base directory this crate's tests write under: `target/`, inside the checkout.
fn scratch_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/comic-model-scratch")
}

/// `scratch_dir(name)`, created.
pub fn scratch_dir_created(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    fs::create_dir_all(&dir).ok();
    dir
}
