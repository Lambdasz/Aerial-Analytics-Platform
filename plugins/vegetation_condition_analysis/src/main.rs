//! Module 6 - RGB Vegetation Condition Analysis Plugin
//!
//! Usage:
//!     main --healthcheck
//!     main --input payload.json --output result.json

use std::process::ExitCode;

use vegetation_condition::{healthcheck_info, run_analysis};

const HELP: &str = "\
Module 6 - RGB Vegetation Condition Analysis Plugin

Usage:
    main --healthcheck                          Verify the plugin can run
    main --input <payload.json> --output <result.json>
                                                Plugin mode";

/// Value following a flag in the argument list.
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
