//! Exercise the DiskLens scanner from a terminal.
//!
//! ```text
//! diskscan scan PATH [--depth N] [--allocated] [--follow] [--no-hidden]
//! diskscan usage PATH
//! ```

use std::env;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use diskscan::{filesystem_usage, format_bytes, format_tree, scan, ScanOptions, SizeMetric};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("diskscan: {message}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let command = args.first().map(String::as_str);
    match command {
        Some("scan") => cmd_scan(&args[1..]),
        Some("usage") => cmd_usage(&args[1..]),
        Some("-h" | "--help" | "help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown command '{other}'. Try `diskscan help`.")),
    }
}

fn cmd_usage(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("usage: diskscan usage PATH")?;
    let usage = filesystem_usage(std::path::Path::new(path)).map_err(|err| err.to_string())?;
    println!("{}", path);
    println!("  total      {}", format_bytes(usage.total_bytes));
    println!("  used       {}", format_bytes(usage.used_bytes));
    println!("  available  {}", format_bytes(usage.available_bytes));
    println!("  free       {}", format_bytes(usage.free_bytes));
    Ok(())
}

fn cmd_scan(args: &[String]) -> Result<(), String> {
    let mut depth = 2usize;
    let mut metric = SizeMetric::UniqueApparent;
    let mut options = ScanOptions::default();
    let mut path: Option<&str> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--depth" => {
                index += 1;
                let value = args.get(index).ok_or("--depth needs a number")?;
                depth = value.parse().map_err(|_| format!("not a depth: {value}"))?;
            }
            "--allocated" => metric = SizeMetric::UniqueAllocated,
            "--logical" => metric = SizeMetric::LogicalApparent,
            "--follow" => options.follow_symlinks = true,
            "--no-hidden" => options.include_hidden = false,
            "--stay" => options.stay_on_filesystem = true,
            "--no-clones" => options.detect_clones = false,
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            other if path.is_none() => path = Some(other),
            other => return Err(format!("unexpected argument {other}")),
        }
        index += 1;
    }
    let path = path.ok_or("usage: diskscan scan PATH")?;
    let cancel = AtomicBool::new(false);
    let tree = scan(path, &options, &cancel, None).map_err(|err| err.to_string())?;
    println!(
        "{}  —  {} unique, {} logical, {} files, {} ms",
        tree.root_path.display(),
        format_bytes(tree.root.unique_apparent),
        format_bytes(tree.root.logical_apparent),
        tree.root.file_count,
        tree.elapsed_ms,
    );
    if !tree.issues.is_empty() {
        println!("{} unreadable entries", tree.issues.len());
    }
    print!("{}", format_tree(&tree.root, metric, depth));
    Ok(())
}

fn print_help() {
    println!(
        "\
DiskLens scanner

  diskscan scan PATH [--depth N] [--allocated|--logical] [--follow]
                     [--no-hidden] [--stay] [--no-clones]
  diskscan usage PATH

scan walks a folder on a thread pool and prints a size tree.
usage prints the volume's total, used, and available space."
    );
}
