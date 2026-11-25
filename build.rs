//! Records the Git commit `hcc --version` prints, when the build has one.
//!
//! `HCC_COMMIT` in the environment wins (a release build can set it);
//! otherwise `git rev-parse --short=12 HEAD` is asked. A build from a
//! source archive with no Git has no commit, and `--version` omits it.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=HCC_COMMIT");
    let commit = std::env::var("HCC_COMMIT").ok().or_else(|| {
        let output = Command::new("git")
            .args(["rev-parse", "--short=12", "HEAD"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        // Rebuild when HEAD moves (a commit, a checkout).
        for path in ["HEAD", "refs/heads"] {
            if let Ok(found) = Command::new("git")
                .args(["rev-parse", "--git-path", path])
                .output()
            {
                let path = String::from_utf8_lossy(&found.stdout).trim().to_string();
                if !path.is_empty() {
                    println!("cargo:rerun-if-changed={path}");
                }
            }
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    });
    if let Some(commit) = commit.filter(|commit| !commit.is_empty()) {
        println!("cargo:rustc-env=HCC_COMMIT={commit}");
    }
}
