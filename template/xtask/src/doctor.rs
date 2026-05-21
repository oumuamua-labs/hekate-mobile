// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use clap::ValueEnum;
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

#[derive(Clone, Copy, ValueEnum)]
pub enum Platform {
    Ios,
    Android,
}

enum Status {
    Ok,
    Fail(String),
}

struct Check {
    name: &'static str,
    status: Status,
}

pub fn run(filter: Option<Platform>) -> ExitCode {
    let ios = [
        xcode_clt(),
        ios_targets(),
        macho_tools(),
        codesign_identity(),
    ];

    let android = [android_ndk(), android_targets(), cargo_ndk(), jdk()];

    let ok = match filter {
        Some(Platform::Ios) => report("iOS", &ios),
        Some(Platform::Android) => report("Android", &android),
        None => {
            let ios_ok = report("iOS", &ios);
            println!();

            let android_ok = report("Android", &android);
            println!();

            match (ios_ok, android_ok) {
                (true, true) => {
                    println!("All prereqs present for iOS and Android.");
                }
                (true, false) => {
                    println!("iOS ready. Android missing prereqs (see above).");
                }
                (false, true) => {
                    println!("Android ready. iOS missing prereqs (see above).");
                }
                (false, false) => {
                    println!("Neither platform is ready (see above).");
                }
            }

            ios_ok && android_ok
        }
    };

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn report(label: &str, checks: &[Check]) -> bool {
    println!("=== {label} ===");

    let mut fails = 0usize;
    for c in checks {
        match &c.status {
            Status::Ok => println!("[ok]   {}", c.name),
            Status::Fail(hint) => {
                println!("[fail] {}", c.name);
                println!("       hint: {hint}");

                fails += 1;
            }
        }
    }

    fails == 0
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
                "install via `brew install android-ndk` then export ANDROID_NDK_HOME=/opt/homebrew/share/android-ndk \
                 (or install via Android Studio → SDK Manager → NDK and point at ~/Library/Android/sdk/ndk/<version>)"
                    .into(),
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

fn jdk() -> Check {
    let name = "JDK 17+ (javac)";
    let out = match Command::new("javac").arg("-version").output() {
        Ok(o) => o,
        Err(_) => {
            return Check {
                name,
                status: Status::Fail(
                    "install Temurin 17+ (`brew install --cask temurin@17`) and set JAVA_HOME"
                        .into(),
                ),
            };
        }
    };

    if !out.status.success() {
        return Check {
            name,
            status: Status::Fail(format!(
                "`javac -version` exited {}; reinstall a working JDK 17+",
                out.status.code().unwrap_or(-1)
            )),
        };
    }

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    match parse_javac_major(&combined) {
        Some(v) if v >= 17 => Check {
            name,
            status: Status::Ok,
        },
        Some(v) => Check {
            name,
            status: Status::Fail(format!(
                "found JDK {v}; gradle Android plugin requires 17+. Install Temurin 17 or newer"
            )),
        },
        None => Check {
            name,
            status: Status::Fail(format!(
                "could not parse `javac -version` output `{}`; install JDK 17+",
                combined.trim()
            )),
        },
    }
}

fn parse_javac_major(s: &str) -> Option<u32> {
    let token = s
        .split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))?;

    let mut parts = token.split('.');
    let first: u32 = parts.next()?.parse().ok()?;

    // JDK <= 8 reports `1.8.0_xxx` / `1.7.0_xxx`; the
    // meaningful major lives in the second component.
    match first {
        1 => parts.next()?.parse().ok(),
        n => Some(n),
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

#[cfg(test)]
mod tests {
    use super::parse_javac_major;

    #[test]
    fn parses_modern_jdk_format() {
        assert_eq!(parse_javac_major("javac 17.0.5"), Some(17));
        assert_eq!(parse_javac_major("javac 21"), Some(21));
        assert_eq!(parse_javac_major("javac 22.0.1\n"), Some(22));
    }

    #[test]
    fn parses_jdk_8_legacy_format() {
        assert_eq!(parse_javac_major("javac 1.8.0_392"), Some(8));
        assert_eq!(parse_javac_major("javac 1.7.0_80"), Some(7));
    }

    #[test]
    fn returns_none_on_unparseable_input() {
        assert_eq!(parse_javac_major(""), None);
        assert_eq!(parse_javac_major("no version here"), None);
    }
}
