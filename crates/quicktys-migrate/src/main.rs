//! `quicktys-migrate` — best-effort CastMate → Quicktys migration CLI (M8).
//!
//! Usage:
//!   quicktys-migrate <castmate-data-dir> [--out <dir>]
//!
//! Reads the user's CastMate YAML resources under `<castmate-data-dir>` and
//! writes Quicktys resources + `migration-report.md` under `--out`
//! (default `./quicktys-migrated`). Nothing is deleted; review the report.

use anyhow::{bail, Result};
use quicktys_migrate::{migrate, Kind};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut src: Option<PathBuf> = None;
    let mut out = PathBuf::from("quicktys-migrated");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                out = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow::anyhow!("--out needs a path"))?;
            }
            "-h" | "--help" => {
                println!("usage: quicktys-migrate <castmate-data-dir> [--out <dir>]");
                return Ok(());
            }
            other if src.is_none() => src = Some(PathBuf::from(other)),
            other => bail!("unexpected argument: {other}"),
        }
    }

    let Some(src) = src else {
        bail!("usage: quicktys-migrate <castmate-data-dir> [--out <dir>]");
    };
    if !src.is_dir() {
        bail!("not a directory: {}", src.display());
    }

    let report = migrate(&src, &out)?;
    println!(
        "migrated {} profile(s), {} automation(s), {} action(s) → {}",
        report.migrated(Kind::Profile),
        report.migrated(Kind::Automation),
        report.action_count(),
        out.display(),
    );
    if !report.errors.is_empty() {
        println!("{} file(s) could not be read (see report)", report.errors.len());
    }
    if report.needs_review() {
        println!("⚠️  some items need manual review — see {}/migration-report.md", out.display());
    } else {
        println!("✓ clean migration — no manual review flagged");
    }
    Ok(())
}
