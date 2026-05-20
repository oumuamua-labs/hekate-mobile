// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::SystemTime;

const SLICES: [&str; 2] = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];
const PROVER_FRAMEWORK: &str = "HekateProverCdylib";
const PROVER_DYLIB_STEM: &str = "libhekate_prover_cdylib";

struct BuildInfo {
    pkg_name: String,
    lib_name: String,
    framework_name: String,
    dev_version: String,
    prover_version: String,
}

pub fn run(unsigned: bool) -> ExitCode {
    let workspace = match find_workspace_root() {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };

    let info = match read_build_info(&workspace) {
        Ok(i) => i,
        Err(e) => return fail(&e),
    };

    let mut metadata_dylib: Option<PathBuf> = None;

    for slice in SLICES {
        println!("=== building {slice} ===");

        if let Err(e) = cross_compile(&workspace, slice, &info.pkg_name) {
            return fail(&format!("{slice}: {e}"));
        }

        let dev_dylib = workspace
            .join("target")
            .join(slice)
            .join("release")
            .join(format!("lib{}.dylib", info.lib_name));

        if !dev_dylib.is_file() {
            return fail(&format!(
                "{slice}: build succeeded but {} is missing",
                dev_dylib.display()
            ));
        }

        let prover_dylib = match find_prover_cdylib(&workspace, slice) {
            Ok(p) => p,
            Err(e) => return fail(&format!("{slice}: {e}")),
        };

        let frameworks_dir = workspace
            .join("target")
            .join(slice)
            .join("release")
            .join("frameworks");

        let prover_old_ref = match read_prover_load_dylib(&dev_dylib) {
            Ok(r) => r,
            Err(e) => return fail(&format!("{slice}: {e}")),
        };

        let prover_fw = match wrap_framework(
            &prover_dylib,
            PROVER_FRAMEWORK,
            &frameworks_dir,
            slice_platform(slice),
            &info.prover_version,
        ) {
            Ok(p) => p,
            Err(e) => return fail(&format!("{slice}: prover framework: {e}")),
        };

        println!("[ok]   {slice}: {}", prover_fw.display());

        let dev_fw = match wrap_framework(
            &dev_dylib,
            &info.framework_name,
            &frameworks_dir,
            slice_platform(slice),
            &info.dev_version,
        ) {
            Ok(p) => p,
            Err(e) => return fail(&format!("{slice}: dev framework: {e}")),
        };

        if let Err(e) = relink_prover_ref(&dev_fw, &info.framework_name, &prover_old_ref) {
            return fail(&format!("{slice}: relink: {e}"));
        }

        println!("[ok]   {slice}: {}", dev_fw.display());

        if metadata_dylib.is_none() {
            metadata_dylib = Some(dev_dylib);
        }
    }

    println!("=== generating swift bindings ===");

    let swift_out = workspace
        .join("target")
        .join("swift")
        .join(&info.framework_name);

    let metadata_source = match metadata_dylib {
        Some(p) => p,
        None => return fail("no slices built"),
    };

    if let Err(e) = generate_swift_bindings(&workspace, &metadata_source, &swift_out) {
        return fail(&format!("swift bindings: {e}"));
    }

    println!("[ok]   swift bindings: {}", swift_out.display());

    println!("=== wiring bindings into dev frameworks ===");

    for slice in SLICES {
        let dev_fw = workspace
            .join("target")
            .join(slice)
            .join("release")
            .join("frameworks")
            .join(format!("{}.framework", info.framework_name));

        if let Err(e) = wire_bindings_into_framework(&dev_fw, &swift_out, &info.lib_name) {
            return fail(&format!("{slice}: wire bindings: {e}"));
        }
    }

    println!("[ok]   bindings wired into dev framework on both slices");
    println!("=== assembling xcframeworks ===");

    let xcf_root = workspace.join("target").join("xcframeworks");
    if xcf_root.exists()
        && let Err(e) = std::fs::remove_dir_all(&xcf_root)
    {
        return fail(&format!("clean {}: {e}", xcf_root.display()));
    }

    if let Err(e) = std::fs::create_dir_all(&xcf_root) {
        return fail(&format!("mkdir {}: {e}", xcf_root.display()));
    }

    let prover_xcf = match assemble_xcframework(&workspace, PROVER_FRAMEWORK, &xcf_root) {
        Ok(p) => p,
        Err(e) => return fail(&format!("prover xcframework: {e}")),
    };

    println!("[ok]   {}", prover_xcf.display());

    let dev_xcf = match assemble_xcframework(&workspace, &info.framework_name, &xcf_root) {
        Ok(p) => p,
        Err(e) => return fail(&format!("dev xcframework: {e}")),
    };

    println!("[ok]   {}", dev_xcf.display());

    if unsigned {
        eprintln!("[warn] xcframeworks emitted UNSIGNED; not suitable for App Store distribution");
    } else {
        let team_id = match env::var("HEKATE_IOS_TEAM_ID") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                return fail("HEKATE_IOS_TEAM_ID not set; pass --unsigned for unsigned dev builds");
            }
        };

        println!("=== codesigning ===");

        if let Err(e) = codesign(&prover_xcf, &team_id) {
            return fail(&format!("sign prover: {e}"));
        }

        println!("[ok]   signed {}", prover_xcf.display());

        if let Err(e) = codesign(&dev_xcf, &team_id) {
            return fail(&format!("sign dev: {e}"));
        }

        println!("[ok]   signed {}", dev_xcf.display());
    }

    println!();
    println!(
        "Built {} iOS slice(s), 2 xcframeworks at {}, Swift bindings at {}.",
        SLICES.len(),
        xcf_root.display(),
        swift_out.display()
    );

    ExitCode::SUCCESS
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR not set; run via `cargo xtask ios-build`".to_string())?;
    let xtask_dir = PathBuf::from(manifest_dir);
    let workspace = xtask_dir
        .parent()
        .ok_or_else(|| format!("xtask dir has no parent: {}", xtask_dir.display()))?
        .to_path_buf();

    if !workspace.join("Cargo.toml").is_file() {
        return Err(format!(
            "workspace Cargo.toml not found at {}",
            workspace.join("Cargo.toml").display()
        ));
    }

    Ok(workspace)
}

fn read_build_info(workspace: &Path) -> Result<BuildInfo, String> {
    let manifest = workspace.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let parsed: toml::Value =
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", manifest.display()))?;

    let package = parsed
        .get("package")
        .ok_or_else(|| format!("no [package] table in {}", manifest.display()))?;

    let pkg_name = package
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| "no [package].name in workspace Cargo.toml".to_string())?
        .to_string();

    let dev_version = package
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no [package].version in workspace Cargo.toml".to_string())?
        .to_string();

    let lib_name = parsed
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| pkg_name.replace('-', "_"));

    let prover_version = parse_prover_version(&parsed)?;

    Ok(BuildInfo {
        framework_name: pascalize(&pkg_name),
        pkg_name,
        lib_name,
        dev_version,
        prover_version,
    })
}

fn parse_prover_version(manifest: &toml::Value) -> Result<String, String> {
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get("hekate-prover-sys"))
        .ok_or_else(|| "[dependencies].hekate-prover-sys missing".to_string())?;

    let raw = match dep {
        toml::Value::String(s) => s.as_str(),
        toml::Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "[dependencies.hekate-prover-sys].version missing".to_string())?,
        _ => return Err("[dependencies].hekate-prover-sys has unexpected shape".into()),
    };

    Ok(raw.trim_start_matches('=').trim().to_string())
}

fn pascalize(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            let head = chars.next().unwrap().to_ascii_uppercase();
            let tail: String = chars.map(|c| c.to_ascii_lowercase()).collect();

            format!("{head}{tail}")
        })
        .collect()
}

fn cross_compile(workspace: &Path, target: &str, pkg_name: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            target,
            "-p",
            pkg_name,
            "--lib",
        ])
        .current_dir(workspace)
        .status()
        .map_err(|e| format!("spawn cargo: {e}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!(
            "cargo build exited {}",
            status.code().unwrap_or(-1)
        )),
    }
}

fn find_prover_cdylib(workspace: &Path, slice: &str) -> Result<PathBuf, String> {
    let build_dir = workspace
        .join("target")
        .join(slice)
        .join("release")
        .join("build");

    if !build_dir.is_dir() {
        return Err(format!("build dir missing: {}", build_dir.display()));
    }

    let entries = std::fs::read_dir(&build_dir)
        .map_err(|e| format!("read_dir {}: {e}", build_dir.display()))?;

    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();

        if !name.to_string_lossy().starts_with("hekate-prover-sys-") {
            continue;
        }

        let candidate = entry
            .path()
            .join("out")
            .join(format!("{PROVER_DYLIB_STEM}.dylib"));

        if !candidate.is_file() {
            continue;
        }

        let mtime = candidate
            .metadata()
            .and_then(|m| m.modified())
            .map_err(|e| format!("stat {}: {e}", candidate.display()))?;

        match &best {
            Some((best_mtime, _)) if *best_mtime >= mtime => {}
            _ => best = Some((mtime, candidate)),
        }
    }

    best.map(|(_, p)| p).ok_or_else(|| {
        format!(
            "{PROVER_DYLIB_STEM}.dylib not found under {}",
            build_dir.display()
        )
    })
}

fn read_prover_load_dylib(dev_dylib: &Path) -> Result<String, String> {
    let out = Command::new("otool")
        .args(["-L", dev_dylib.to_string_lossy().as_ref()])
        .output()
        .map_err(|e| format!("spawn otool: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "otool -L exited {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains(PROVER_DYLIB_STEM) {
            let path = trimmed
                .split_whitespace()
                .next()
                .ok_or_else(|| format!("could not split otool line: {trimmed}"))?;

            return Ok(path.to_string());
        }
    }

    Err(format!(
        "{PROVER_DYLIB_STEM} not referenced in {}",
        dev_dylib.display()
    ))
}

fn wrap_framework(
    dylib_src: &Path,
    name: &str,
    output_root: &Path,
    platform: &str,
    version: &str,
) -> Result<PathBuf, String> {
    let fw_dir = output_root.join(format!("{name}.framework"));
    if fw_dir.exists() {
        std::fs::remove_dir_all(&fw_dir).map_err(|e| format!("clean {}: {e}", fw_dir.display()))?;
    }

    std::fs::create_dir_all(&fw_dir).map_err(|e| format!("mkdir {}: {e}", fw_dir.display()))?;

    let bin_dst = fw_dir.join(name);
    std::fs::copy(dylib_src, &bin_dst)
        .map_err(|e| format!("copy {} -> {}: {e}", dylib_src.display(), bin_dst.display()))?;

    let plist = info_plist(name, platform, version);
    std::fs::write(fw_dir.join("Info.plist"), plist)
        .map_err(|e| format!("write Info.plist in {}: {e}", fw_dir.display()))?;

    let install_id = format!("@rpath/{name}.framework/{name}");
    run_install_name_tool(&["-id", &install_id, bin_dst.to_string_lossy().as_ref()])?;

    Ok(fw_dir)
}

fn relink_prover_ref(dev_fw: &Path, dev_name: &str, old_ref: &str) -> Result<(), String> {
    let bin = dev_fw.join(dev_name);
    let new_ref = format!("@rpath/{PROVER_FRAMEWORK}.framework/{PROVER_FRAMEWORK}");

    run_install_name_tool(&["-change", old_ref, &new_ref, bin.to_string_lossy().as_ref()])
}

fn generate_swift_bindings(workspace: &Path, dylib: &Path, out_dir: &Path) -> Result<(), String> {
    if out_dir.exists() {
        std::fs::remove_dir_all(out_dir)
            .map_err(|e| format!("clean {}: {e}", out_dir.display()))?;
    }

    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    let status = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--bin",
            "uniffi-bindgen",
            "--",
            "generate",
            "--library",
            dylib.to_string_lossy().as_ref(),
            "--language",
            "swift",
            "--out-dir",
            out_dir.to_string_lossy().as_ref(),
        ])
        .current_dir(workspace)
        .status()
        .map_err(|e| format!("spawn cargo run: {e}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!(
            "uniffi-bindgen exited {}",
            status.code().unwrap_or(-1)
        )),
    }
}

fn run_install_name_tool(args: &[&str]) -> Result<(), String> {
    let status = Command::new("install_name_tool")
        .args(args)
        .status()
        .map_err(|e| format!("spawn install_name_tool: {e}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!(
            "install_name_tool {args:?} exited {}",
            status.code().unwrap_or(-1)
        )),
    }
}

fn slice_platform(slice: &str) -> &'static str {
    match slice {
        "aarch64-apple-ios-sim" | "x86_64-apple-ios" => "iPhoneSimulator",
        _ => "iPhoneOS",
    }
}

fn info_plist(name: &str, platform: &str, version: &str) -> String {
    let bundle_version = sanitize_bundle_version(version);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundleIdentifier</key>
    <string>dev.oumuamua.hekate.{lower}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleVersion</key>
    <string>{bundle_version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{bundle_version}</string>
    <key>MinimumOSVersion</key>
    <string>16.0</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>{platform}</string>
    </array>
</dict>
</plist>
"#,
        lower = name.to_ascii_lowercase()
    )
}

fn sanitize_bundle_version(raw: &str) -> String {
    let core = raw.split(['-', '+']).next().unwrap_or(raw);
    let kept: String = core
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();

    if kept.is_empty() || kept.chars().all(|c| c == '.') {
        return "0".to_string();
    }

    kept
}

fn wire_bindings_into_framework(
    framework: &Path,
    swift_out: &Path,
    lib_name: &str,
) -> Result<(), String> {
    let header_src = swift_out.join(format!("{lib_name}FFI.h"));
    let modulemap_src = swift_out.join(format!("{lib_name}FFI.modulemap"));

    if !header_src.is_file() {
        return Err(format!("missing {}", header_src.display()));
    }

    if !modulemap_src.is_file() {
        return Err(format!("missing {}", modulemap_src.display()));
    }

    let headers_dir = framework.join("Headers");
    let modules_dir = framework.join("Modules");

    std::fs::create_dir_all(&headers_dir)
        .map_err(|e| format!("mkdir {}: {e}", headers_dir.display()))?;
    std::fs::create_dir_all(&modules_dir)
        .map_err(|e| format!("mkdir {}: {e}", modules_dir.display()))?;

    let header_dst = headers_dir.join(format!("{lib_name}FFI.h"));
    std::fs::copy(&header_src, &header_dst).map_err(|e| format!("copy header: {e}"))?;

    let modulemap_text = std::fs::read_to_string(&modulemap_src)
        .map_err(|e| format!("read {}: {e}", modulemap_src.display()))?;
    let framework_modulemap = rewrite_modulemap_as_framework(&modulemap_text)?;

    std::fs::write(modules_dir.join("module.modulemap"), framework_modulemap)
        .map_err(|e| format!("write module.modulemap: {e}"))?;

    Ok(())
}

fn rewrite_modulemap_as_framework(text: &str) -> Result<String, String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with("framework module") {
        return Ok(text.to_string());
    }

    if let Some(rest) = trimmed.strip_prefix("module ") {
        return Ok(format!("framework module {rest}"));
    }

    let preview: String = trimmed.chars().take(64).collect();

    Err(format!(
        "uniffi modulemap format unrecognized; expected leading `module ` or `framework module `, \
         got `{preview}`. Verify the pinned uniffi crate version and update \
         rewrite_modulemap_as_framework in template/xtask/src/ios_build.rs."
    ))
}

fn assemble_xcframework(
    workspace: &Path,
    framework_name: &str,
    out_dir: &Path,
) -> Result<PathBuf, String> {
    let xcf_path = out_dir.join(format!("{framework_name}.xcframework"));

    let mut args: Vec<String> = vec!["-create-xcframework".into()];
    for slice in SLICES {
        let slice_fw = workspace
            .join("target")
            .join(slice)
            .join("release")
            .join("frameworks")
            .join(format!("{framework_name}.framework"));

        if !slice_fw.is_dir() {
            return Err(format!("missing {}", slice_fw.display()));
        }

        args.push("-framework".into());
        args.push(slice_fw.to_string_lossy().into_owned());
    }

    args.push("-output".into());
    args.push(xcf_path.to_string_lossy().into_owned());

    let status = Command::new("xcodebuild")
        .args(&args)
        .status()
        .map_err(|e| format!("spawn xcodebuild: {e}"))?;

    match status.success() {
        true => Ok(xcf_path),
        false => Err(format!("xcodebuild exited {}", status.code().unwrap_or(-1))),
    }
}

fn codesign(xcframework: &Path, identity: &str) -> Result<(), String> {
    let status = Command::new("codesign")
        .args([
            "--sign",
            identity,
            "--timestamp",
            xcframework.to_string_lossy().as_ref(),
        ])
        .status()
        .map_err(|e| format!("spawn codesign: {e}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!("codesign exited {}", status.code().unwrap_or(-1))),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("[fail] {msg}");
    ExitCode::from(1)
}
