//! Generates the bundled frontend assets that `src/lib.rs` embeds with
//! `include_str!`, so a fresh clone builds without a manual `bun run build`.
//!
//! `static/app.js`, `static/app.css` and `static/pip.js` are build artifacts and
//! are gitignored; this script rebuilds them from `web/src` and `styles/app.css`
//! whenever those sources change.

use std::path::{Path, PathBuf};
use std::process::Command;

const GENERATED: &[&str] = &["static/app.js", "static/app.css", "static/pip.js"];

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Only the frontend sources trigger a rebuild. The generated files under
    // `static/` are outputs, so watching them would loop.
    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=styles/app.css");
    println!("cargo:rerun-if-changed=static/index.html");
    println!("cargo:rerun-if-changed=static/pip.html");
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=bun.lock");

    let missing: Vec<&str> = GENERATED
        .iter()
        .copied()
        .filter(|rel| !root.join(rel).exists())
        .collect();

    let Some(bun) = find_bun() else {
        if missing.is_empty() {
            println!("cargo:warning=bun not found on PATH; reusing existing static/ bundles");
            return;
        }
        panic!(
            "bun is required to build the frontend bundles ({}). \
             Install bun (https://bun.sh) or run `bun run build` on a machine that has it.",
            missing.join(", ")
        );
    };

    if !root.join("node_modules").exists() {
        run(&bun, &["install"], &root);
    }
    run(&bun, &["run", "build"], &root);

    for rel in GENERATED {
        if !root.join(rel).exists() {
            panic!("`bun run build` did not produce {rel}");
        }
    }
}

fn find_bun() -> Option<String> {
    for candidate in ["bun", "bun.exe"] {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn run(bun: &str, args: &[&str], cwd: &Path) {
    let status = Command::new(bun)
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{bun} {}`: {e}", args.join(" ")));
    if !status.success() {
        panic!("`{bun} {}` failed with {status}", args.join(" "));
    }
}
