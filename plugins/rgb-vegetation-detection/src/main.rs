//! Module 4 - RGB Vegetation Detection Plugin
//!
//! Cara pakai:
//!     main --healthcheck
//!     main --input payload.json --output hasil.json

use std::process::ExitCode;

use rgb_vegetation::{healthcheck_info, run_analysis};

const HELP: &str = "\
Module 4 - RGB Vegetation Detection Plugin

Usage:
    main --healthcheck                          Cek plugin siap jalan
    main --input <payload.json> --output <result.json>
                                                Mode plugin";

/// Nilai argumen setelah flag tertentu.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--healthcheck") {
        println!("{}", healthcheck_info());
        return ExitCode::SUCCESS;
    }

    match (flag_value(&args, "--input"), flag_value(&args, "--output")) {
        (Some(input), Some(output)) => ExitCode::from(run_analysis(input, output) as u8),
        _ => {
            println!("{HELP}");
            ExitCode::FAILURE
        }
    }
}
