// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod android_build;
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
    Doctor {
        /// Scope checks to a single platform.
        /// Omit to inspect both.
        #[arg(value_enum)]
        platform: Option<doctor::Platform>,
    },

    /// Cross-compile, wrap frameworks, generate
    /// Swift bindings, and assemble xcframeworks
    /// for iOS device + simulator slices.
    IosBuild {
        /// Skip codesigning. CI only, App Store
        /// rejects unsigned xcframeworks.
        #[arg(long)]
        unsigned: bool,
    },

    /// Cross-compile via cargo-ndk, generate
    /// Kotlin bindings, bundle the prover cdylib
    /// into jniLibs, and assemble an .aar via
    /// gradle.
    AndroidBuild,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Doctor { platform } => doctor::run(platform),
        Command::IosBuild { unsigned } => ios_build::run(unsigned),
        Command::AndroidBuild => android_build::run(),
    }
}
