use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use econfcheck::check::check_content;
use econfcheck::parse::{effective_properties, parse};

const SKIP_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", "vendor", "dist", "build"];

#[derive(Parser)]
#[command(
    name = "econfcheck",
    about = "Validates real files against a directory's own .editorconfig rules"
)]
struct Cli {
    /// Directory to scan. Its own .editorconfig is read from here.
    #[arg(default_value = ".")]
    dir: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let econfig_path = cli.dir.join(".editorconfig");
    let content = match fs::read_to_string(&econfig_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("econfcheck: reading {}: {e}", econfig_path.display());
            return ExitCode::FAILURE;
        }
    };
    let config = parse(&content);

    let mut files = Vec::new();
    if let Err(e) = walk(&cli.dir, &cli.dir, &mut files) {
        eprintln!("econfcheck: {e}");
        return ExitCode::FAILURE;
    }

    let mut total_violations = 0usize;
    for (full_path, rel_path) in &files {
        let props = effective_properties(&config, rel_path);
        if props.is_empty() {
            continue;
        }
        let Ok(raw) = fs::read(full_path) else {
            continue; // unreadable file (permissions, etc.) — skipped, not fatal
        };
        let Ok(text) = String::from_utf8(raw.clone()) else {
            continue; // binary file — nothing in this tool's rule set applies to it
        };
        let violations = check_content(&text, &raw, &props);
        for v in violations {
            total_violations += 1;
            match v.line {
                Some(line) => println!("{rel_path}:{line}: {} — {}", v.rule, v.message),
                None => println!("{rel_path}: {} — {}", v.rule, v.message),
            }
        }
    }

    if total_violations == 0 {
        println!("econfcheck: clean, no violations found");
        ExitCode::SUCCESS
    } else {
        eprintln!("\neconfcheck: {total_violations} violation(s) found");
        ExitCode::FAILURE
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIR_NAMES.contains(&name.as_ref()) {
                continue;
            }
            walk(root, &path, out)?;
        } else if path.is_file() {
            if name == ".editorconfig" {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((path.clone(), rel));
        }
    }
    Ok(())
}
