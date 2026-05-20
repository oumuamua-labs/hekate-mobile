// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod doctor;
mod ios_build;

#[derive(Parser)]
#[command(name = "xtask", about = "Hekate mobile build orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify host-machine prerequisites
    /// for iOS and Android builds.
    Doctor,

    /// Cross-compile, wrap frameworks, generate
    /// Swift bindings, and assemble xcframeworks
    /// for iOS device + simulator slices.
    IosBuild {
        /// Skip codesigning. CI only, App Store
        /// rejects unsigned xcframeworks.
        #[arg(long)]
        unsigned: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Doctor => doctor::run(),
        Command::IosBuild { unsigned } => ios_build::run(unsigned),
    }
}
