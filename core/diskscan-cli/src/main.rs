//! Exercise the DiskLens scanner from a terminal.
//!
//! ```text
//! diskscan scan PATH [--depth N] [--allocated] [--follow] [--no-hidden]
//! diskscan usage PATH
//! ```

use std::env;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use diskscan::{
    filesystem_usage, find_duplicates, format_bytes, format_tree, scan, DuplicateOptions,
    ScanOptions, SizeMetric,
};

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
        Some("dupes") => cmd_dupes(&args[1..]),
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

struct ScanArgs<'a> {
    path: &'a str,
    depth: usize,
    metric: SizeMetric,
    options: ScanOptions,
}

fn parse_scan(args: &[String]) -> Result<ScanArgs<'_>, String> {
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
    Ok(ScanArgs { path, depth, metric, options })
}

fn cmd_scan(args: &[String]) -> Result<(), String> {
    let parsed = parse_scan(args)?;
    let ScanArgs { path, depth, metric, options } = parsed;
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

struct DupesArgs<'a> {
    path: &'a str,
    options: DuplicateOptions,
}

fn parse_dupes(args: &[String]) -> Result<DupesArgs<'_>, String> {
    let mut options = DuplicateOptions::default();
    let mut path: Option<&str> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--min-size" => {
                index += 1;
                let value = args.get(index).ok_or("--min-size needs a number of bytes")?;
                options.min_size = value.parse().map_err(|_| format!("not a size: {value}"))?;
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            other if path.is_none() => path = Some(other),
            other => return Err(format!("unexpected argument {other}")),
        }
        index += 1;
    }
    let path = path.ok_or("usage: diskscan dupes PATH [--min-size BYTES]")?;
    Ok(DupesArgs { path, options })
}

fn cmd_dupes(args: &[String]) -> Result<(), String> {
    let parsed = parse_dupes(args)?;
    let path = parsed.path;
    let options = parsed.options;
    let cancel = AtomicBool::new(false);
    let report = find_duplicates(path, &options, &cancel, None).map_err(|err| err.to_string())?;
    println!(
        "{} groups, {} files considered, {} hashed",
        report.groups.len(),
        report.files_considered,
        report.files_hashed
    );
    for group in &report.groups {
        println!();
        println!(
            "{} × {}  ({})",
            group.files.len(),
            format_bytes(group.size),
            format_bytes(group.wasted_bytes())
        );
        for file in &group.files {
            println!("  {}", file.path.display());
        }
    }
    if !report.unreadable.is_empty() {
        println!();
        println!("{} files could not be read", report.unreadable.len());
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
DiskLens scanner

  diskscan scan PATH [--depth N] [--allocated|--logical] [--follow]
                     [--no-hidden] [--stay] [--no-clones]
  diskscan dupes PATH [--min-size BYTES]
  diskscan usage PATH

scan walks a folder on a thread pool and prints a size tree.
dupes prints files that share a size and a BLAKE3 hash.
usage prints the volume's total, used, and available space."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_flags_are_parsed_without_walking() {
        let args = vec![
            "/tmp/photos".into(),
            "--depth".into(),
            "4".into(),
            "--logical".into(),
            "--follow".into(),
            "--no-hidden".into(),
            "--stay".into(),
            "--no-clones".into(),
        ];
        let parsed = parse_scan(&args).unwrap();
        assert_eq!(parsed.path, "/tmp/photos");
        assert_eq!(parsed.depth, 4);
        assert_eq!(parsed.metric, SizeMetric::LogicalApparent);
        assert!(parsed.options.follow_symlinks);
        assert!(!parsed.options.include_hidden);
        assert!(parsed.options.stay_on_filesystem);
        assert!(!parsed.options.detect_clones);
    }

    #[test]
    fn scan_rejects_a_missing_depth_and_an_unknown_flag() {
        assert!(parse_scan(&["/tmp".into(), "--depth".into()]).is_err());
        assert!(parse_scan(&["/tmp".into(), "--nope".into()]).is_err());
        assert!(parse_scan(&["/tmp".into(), "extra".into()]).is_err());
        assert!(parse_scan(&[]).is_err());
    }

    #[test]
    fn dupes_min_size_parses_and_unknown_commands_fail() {
        let args = vec!["/tmp".into(), "--min-size".into(), "4096".into()];
        let parsed = parse_dupes(&args).unwrap();
        assert_eq!(parsed.path, "/tmp");
        assert_eq!(parsed.options.min_size, 4096);
        assert!(parse_dupes(&["--min-size".into(), "nope".into()]).is_err());
        assert!(run(vec!["paint".into()]).is_err());
        assert!(run(vec!["help".into()]).is_ok());
    }
}
