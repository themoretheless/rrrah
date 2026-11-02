//! Read-only exact duplicate scan for one or more folders.
use rrrah_dedup::exact::{Options, scan_verified};
use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let mut roots = Vec::new();
    let mut positional = false;
    for arg in env::args_os().skip(1) {
        if !positional && arg == "--" {
            positional = true;
        } else if !positional && (arg == "--help" || arg == "-h") {
            println!("Usage: scan_folder [--] FOLDER [FOLDER ...]");
            println!("Find byte-confirmed duplicate files recursively; never delete files.");
            println!("Symlinks are skipped. Limits: 1,000,000 entries and depth 256.");
            println!("Exit: 0 complete, 1 incomplete scan, 2 missing folder argument.");
            return ExitCode::SUCCESS;
        } else {
            roots.push(PathBuf::from(arg));
        }
    }
    if roots.is_empty() {
        eprintln!("Usage: scan_folder [--] FOLDER [FOLDER ...]");
        return ExitCode::from(2);
    }

    let report = scan_verified(&roots, &Options::default(), || false);
    for (index, group) in report.groups.iter().enumerate() {
        println!(
            "Group {}: {} files, {} bytes each",
            index + 1,
            group.paths.len(),
            group.bytes
        );
        for path in &group.paths {
            println!("  {path:?}");
        }
    }
    for alias in &report.aliases {
        println!(
            "Same physical file (aliases, not separate copies): {:?}",
            alias.paths
        );
    }
    for issue in &report.issues {
        eprintln!("Diagnostic: {:?}: {:?}", issue.path, issue.kind);
    }
    let complete = report.complete();
    eprintln!(
        "Scan {}: {} files observed, {} duplicate groups, {} bytes read, {} diagnostics",
        if complete { "complete" } else { "incomplete" },
        report.files_observed,
        report.groups.len(),
        report.bytes_read,
        report.issues.len(),
    );
    if complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
