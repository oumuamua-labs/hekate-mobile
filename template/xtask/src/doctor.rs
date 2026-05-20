// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

enum Status {
    Ok,
    Fail(String),
}

struct Check {
    name: &'static str,
    status: Status,
}

pub fn run() -> ExitCode {
    let checks = [
        xcode_clt(),
        ios_targets(),
        android_ndk(),
        android_targets(),
        cargo_ndk(),
        macho_tools(),
        codesign_identity(),
    ];

    let mut fails = 0usize;
    for c in &checks {
        match &c.status {
            Status::Ok => println!("[ok]   {}", c.name),
            Status::Fail(hint) => {
                println!("[fail] {}", c.name);
                println!("       hint: {hint}");

                fails += 1;
            }
        }
    }

    println!();

    if fails == 0 {
        println!("All {} prereqs present.", checks.len());
        ExitCode::SUCCESS
    } else {
        println!("{} of {} check(s) failed.", fails, checks.len());
        ExitCode::from(1)
    }
}

fn xcode_clt() -> Check {
    let name = "Xcode Command Line Tools";
    match Command::new("xcode-select").arg("-p").output() {
        Ok(o) if o.status.success() => Check {
            name,
            status: Status::Ok,
        },
        _ => Check {
            name,
            status: Status::Fail(
                "install via `xcode-select --install` (or full Xcode from the App Store)".into(),
            ),
        },
    }
}

fn ios_targets() -> Check {
    let name = "iOS Rust targets (aarch64-apple-ios + sim)";
    let want = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];

    let installed = match rustup_installed_targets() {
        Some(s) => s,
        None => {
            return Check {
                name,
                status: Status::Fail("`rustup` not on PATH; install from https://rustup.rs".into()),
            };
        }
    };

    let missing: Vec<&str> = want
        .iter()
        .copied()
        .filter(|t| !installed.contains(*t))
        .collect();
    if missing.is_empty() {
        Check {
            name,
            status: Status::Ok,
        }
    } else {
        Check {
            name,
            status: Status::Fail(format!("install: rustup target add {}", missing.join(" "))),
        }
    }
}

fn android_ndk() -> Check {
    let name = "Android NDK";
    match env::var("ANDROID_NDK_HOME") {
        Ok(p) if Path::new(&p).is_dir() => Check { name, status: Status::Ok },
        Ok(p) => Check {
            name,
            status: Status::Fail(format!(
                "ANDROID_NDK_HOME points at `{p}` but the directory does not exist; reinstall NDK or fix env var"
            )),
        },
        Err(_) => Check {
            name,
            status: Status::Fail(
                "set ANDROID_NDK_HOME to the NDK install dir (e.g. ~/Library/Android/sdk/ndk/<version>)".into(),
            ),
        },
    }
}

fn android_targets() -> Check {
    let name = "Android Rust target (aarch64-linux-android required)";
    let required = "aarch64-linux-android";

    let installed = match rustup_installed_targets() {
        Some(s) => s,
        None => {
            return Check {
                name,
                status: Status::Fail("`rustup` not on PATH; install from https://rustup.rs".into()),
            };
        }
    };

    if installed.contains(required) {
        Check {
            name,
            status: Status::Ok,
        }
    } else {
        Check {
            name,
            status: Status::Fail(format!("install via `rustup target add {required}`")),
        }
    }
}

fn cargo_ndk() -> Check {
    let name = "cargo-ndk";
    match Command::new("cargo").args(["ndk", "--version"]).output() {
        Ok(o) if o.status.success() => Check {
            name,
            status: Status::Ok,
        },
        _ => Check {
            name,
            status: Status::Fail("install via `cargo install cargo-ndk`".into()),
        },
    }
}

fn macho_tools() -> Check {
    let name = "Mach-O tools (lipo + install_name_tool)";

    let mut missing = Vec::new();
    for tool in ["lipo", "install_name_tool"] {
        let on_path = Command::new("which")
            .arg(tool)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !on_path {
            missing.push(tool);
        }
    }

    if missing.is_empty() {
        Check {
            name,
            status: Status::Ok,
        }
    } else {
        Check {
            name,
            status: Status::Fail(format!(
                "missing: {} — install Xcode CLT via `xcode-select --install`",
                missing.join(", ")
            )),
        }
    }
}

fn codesign_identity() -> Check {
    let name = "Codesigning identity + HEKATE_IOS_TEAM_ID";
    let identity_ok = match Command::new("security")
        .args(["find-identity", "-p", "codesigning", "-v"])
        .output()
    {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            !stdout.contains("0 valid identities found")
        }
        _ => false,
    };

    let team_id = env::var("HEKATE_IOS_TEAM_ID")
        .ok()
        .filter(|s| !s.is_empty());

    match (identity_ok, team_id) {
        (true, Some(_)) => Check { name, status: Status::Ok },
        (false, _) => Check {
            name,
            status: Status::Fail(
                "no codesigning identity in keychain; sign in to Apple Developer account in Xcode → Settings → Accounts".into(),
            ),
        },
        (true, None) => Check {
            name,
            status: Status::Fail(
                "HEKATE_IOS_TEAM_ID env var not set; export your 10-character Apple Developer Team ID".into(),
            ),
        },
    }
}

fn rustup_installed_targets() -> Option<HashSet<String>> {
    let out = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let s = String::from_utf8(out.stdout).ok()?;

    Some(
        s.lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
    )
}
