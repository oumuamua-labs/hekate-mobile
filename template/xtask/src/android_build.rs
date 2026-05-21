// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::SystemTime;

const PROVER_DYLIB_STEM: &str = "libhekate_prover_cdylib";
const PROVER_LIB_NAME: &str = "hekate_prover_cdylib";

const ANDROID_ABIS: &[(&str, &str)] = &[("arm64-v8a", "aarch64-linux-android")];

// Anchored to uniffi 0.31.1 Kotlin codegen, if
// `uniffi` is bumped in Cargo.toml, regenerate
// sample output and update this literal.
const KOTLIN_INJECTION_ANCHOR: &str = "internal object IntegrityCheckingUniffiLib {\n    init {\n        Native.register(IntegrityCheckingUniffiLib::class.java";

const KOTLIN_INJECTION_REPLACEMENT: &str = "internal object IntegrityCheckingUniffiLib {\n    init {\n        System.loadLibrary(\"hekate_prover_cdylib\")\n        Native.register(IntegrityCheckingUniffiLib::class.java";

struct BuildInfo {
    pkg_name: String,
    lib_name: String,
}

pub fn run() -> ExitCode {
    let workspace = match find_workspace_root() {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };

    let info = match read_build_info(&workspace) {
        Ok(i) => i,
        Err(e) => return fail(&e),
    };

    let android_dir = workspace.join("android");
    if !android_dir.is_dir() {
        return fail(&format!(
            "android module missing at {}; expected gradle scaffold from the template",
            android_dir.display()
        ));
    }

    let main_dir = android_dir.join("lib").join("src").join("main");
    let jni_libs_dir = main_dir.join("jniLibs");
    let kotlin_dir = main_dir.join("kotlin");

    if let Err(e) = reset_dir(&jni_libs_dir) {
        return fail(&format!("reset {}: {e}", jni_libs_dir.display()));
    }

    if let Err(e) = reset_dir(&kotlin_dir) {
        return fail(&format!("reset {}: {e}", kotlin_dir.display()));
    }

    let mut metadata_so: Option<PathBuf> = None;

    for (abi, triple) in ANDROID_ABIS {
        println!("=== building {abi} ({triple}) ===");

        if let Err(e) = cargo_ndk_build(&workspace, abi, &info.pkg_name) {
            return fail(&format!("{triple}: {e}"));
        }

        let dev_so = workspace
            .join("target")
            .join(triple)
            .join("release")
            .join(format!("lib{}.so", info.lib_name));

        if !dev_so.is_file() {
            return fail(&format!(
                "{triple}: build succeeded but {} is missing",
                dev_so.display()
            ));
        }

        let prover_so = match find_prover_cdylib(&workspace, triple) {
            Ok(p) => p,
            Err(e) => return fail(&format!("{triple}: {e}")),
        };

        let abi_dir = jni_libs_dir.join(abi);
        if let Err(e) = std::fs::create_dir_all(&abi_dir) {
            return fail(&format!("mkdir {}: {e}", abi_dir.display()));
        }

        let dev_so_dst = abi_dir.join(format!("lib{}.so", info.lib_name));
        if let Err(e) = std::fs::copy(&dev_so, &dev_so_dst) {
            return fail(&format!(
                "copy {} -> {}: {e}",
                dev_so.display(),
                dev_so_dst.display()
            ));
        }

        let prover_so_dst = abi_dir.join(format!("{PROVER_DYLIB_STEM}.so"));
        if let Err(e) = std::fs::copy(&prover_so, &prover_so_dst) {
            return fail(&format!(
                "copy {} -> {}: {e}",
                prover_so.display(),
                prover_so_dst.display()
            ));
        }

        println!("[ok]   {abi}: {}", abi_dir.display());

        if metadata_so.is_none() {
            metadata_so = Some(dev_so);
        }
    }

    let metadata_source = match metadata_so {
        Some(p) => p,
        None => return fail("no ABIs built"),
    };

    println!("=== generating kotlin bindings ===");

    if let Err(e) = generate_kotlin_bindings(&workspace, &metadata_source, &kotlin_dir) {
        return fail(&format!("kotlin bindings: {e}"));
    }

    let kt_file = match find_generated_kotlin(&kotlin_dir) {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };

    println!("[ok]   kotlin bindings: {}", kt_file.display());

    if let Err(e) = inject_prover_load_call(&kt_file) {
        return fail(&format!("inject loadLibrary: {e}"));
    }

    println!(
        "[ok]   injected System.loadLibrary(\"{PROVER_LIB_NAME}\") into {}",
        kt_file.display()
    );

    println!("=== assembling aar via gradle ===");

    if let Err(e) = gradle_assemble(&android_dir) {
        return fail(&format!("gradle: {e}"));
    }

    let gradle_aar = android_dir
        .join("lib")
        .join("build")
        .join("outputs")
        .join("aar")
        .join("lib-release.aar");

    if !gradle_aar.is_file() {
        return fail(&format!(
            "gradle reported success but {} is missing",
            gradle_aar.display()
        ));
    }

    let aar_root = workspace.join("target").join("aar");
    if let Err(e) = std::fs::create_dir_all(&aar_root) {
        return fail(&format!("mkdir {}: {e}", aar_root.display()));
    }

    let aar_out = aar_root.join(format!("{}-release.aar", info.pkg_name));
    if let Err(e) = std::fs::copy(&gradle_aar, &aar_out) {
        return fail(&format!(
            "copy {} -> {}: {e}",
            gradle_aar.display(),
            aar_out.display()
        ));
    }

    println!("[ok]   {}", aar_out.display());

    println!();
    println!(
        "Built {} ABI slice(s), aar at {}.",
        ANDROID_ABIS.len(),
        aar_out.display()
    );

    ExitCode::SUCCESS
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").map_err(|_| {
        "CARGO_MANIFEST_DIR not set; run via `cargo xtask android-build`".to_string()
    })?;
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

    let lib_name = parsed
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| pkg_name.replace('-', "_"));

    Ok(BuildInfo { pkg_name, lib_name })
}

fn reset_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }

    std::fs::create_dir_all(dir).map_err(|e| e.to_string())
}

fn cargo_ndk_build(workspace: &Path, abi: &str, pkg_name: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .args([
            "ndk",
            "-t",
            abi,
            "build",
            "--release",
            "-p",
            pkg_name,
            "--lib",
        ])
        .current_dir(workspace)
        .status()
        .map_err(|e| format!("spawn cargo ndk: {e}"))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!("cargo ndk exited {}", status.code().unwrap_or(-1))),
    }
}

fn find_prover_cdylib(workspace: &Path, triple: &str) -> Result<PathBuf, String> {
    let build_dir = workspace
        .join("target")
        .join(triple)
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
            .join(format!("{PROVER_DYLIB_STEM}.so"));
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
            "{PROVER_DYLIB_STEM}.so not found under {}",
            build_dir.display()
        )
    })
}

fn generate_kotlin_bindings(workspace: &Path, dylib: &Path, out_dir: &Path) -> Result<(), String> {
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
            "kotlin",
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

fn find_generated_kotlin(kotlin_dir: &Path) -> Result<PathBuf, String> {
    let uniffi_root = kotlin_dir.join("uniffi");
    if !uniffi_root.is_dir() {
        return Err(format!(
            "uniffi-bindgen produced no `uniffi/` subdirectory under {}",
            kotlin_dir.display()
        ));
    }

    let mut found: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&uniffi_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        for inner in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
            let inner = inner.map_err(|e| e.to_string())?;
            let p = inner.path();

            if p.extension().and_then(|s| s.to_str()) == Some("kt") {
                found.push(p);
            }
        }
    }

    match found.len() {
        1 => Ok(found.into_iter().next().unwrap()),
        n => Err(format!(
            "expected exactly one .kt under {}, found {n}",
            uniffi_root.display()
        )),
    }
}

fn inject_prover_load_call(kt_file: &Path) -> Result<(), String> {
    let text =
        std::fs::read_to_string(kt_file).map_err(|e| format!("read {}: {e}", kt_file.display()))?;

    let patched =
        inject_prover_load_library(&text).map_err(|e| format!("{}: {e}", kt_file.display()))?;

    std::fs::write(kt_file, patched).map_err(|e| format!("write {}: {e}", kt_file.display()))?;

    Ok(())
}

pub fn inject_prover_load_library(text: &str) -> Result<String, String> {
    let count = text.matches(KOTLIN_INJECTION_ANCHOR).count();
    if count != 1 {
        return Err(format!(
            "uniffi 0.31.1 Kotlin format anchor matched {count} times (expected exactly 1). \
             Verify [dependencies].uniffi pin in the template Cargo.toml and update \
             KOTLIN_INJECTION_ANCHOR in xtask/src/android_build.rs."
        ));
    }

    Ok(text.replacen(KOTLIN_INJECTION_ANCHOR, KOTLIN_INJECTION_REPLACEMENT, 1))
}

fn gradle_assemble(android_dir: &Path) -> Result<(), String> {
    let wrapper = android_dir.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });

    if !wrapper.is_file() {
        return Err(format!(
            "gradle wrapper missing at {}; the template ships it — restore from a fresh `cargo generate`",
            wrapper.display()
        ));
    }

    let status = Command::new(&wrapper)
        .args([":lib:assembleRelease", "--console=plain"])
        .current_dir(android_dir)
        .status()
        .map_err(|e| format!("spawn {}: {e}", wrapper.display()))?;

    match status.success() {
        true => Ok(()),
        false => Err(format!(
            "{} exited {}",
            wrapper.display(),
            status.code().unwrap_or(-1)
        )),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("[fail] {msg}");
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_UNIFFI_FIXTURE: &str = r#"// This file was autogenerated by some hot garbage in the `uniffi` crate.
package uniffi.my_prover_test

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Structure

@Synchronized
private fun findLibraryName(componentName: String): String {
    return "my_prover_test"
}

internal object IntegrityCheckingUniffiLib {
    init {
        Native.register(IntegrityCheckingUniffiLib::class.java, findLibraryName(componentName = "my_prover_test"))
        uniffiCheckContractApiVersion(this)
        uniffiCheckApiChecksums(this)
    }

    external fun ffi_my_prover_test_uniffi_contract_version(): Int
}

internal object UniffiLib {
    init {
        Native.register(UniffiLib::class.java, findLibraryName(componentName = "my_prover_test"))
    }

    external fun uniffi_my_prover_test_fn_func_prove(): Long
}

public fun uniffiEnsureInitialized() {
    IntegrityCheckingUniffiLib
    UniffiLib
}
"#;

    const EXPECTED_INJECTED_FIXTURE: &str = r#"// This file was autogenerated by some hot garbage in the `uniffi` crate.
package uniffi.my_prover_test

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Structure

@Synchronized
private fun findLibraryName(componentName: String): String {
    return "my_prover_test"
}

internal object IntegrityCheckingUniffiLib {
    init {
        System.loadLibrary("hekate_prover_cdylib")
        Native.register(IntegrityCheckingUniffiLib::class.java, findLibraryName(componentName = "my_prover_test"))
        uniffiCheckContractApiVersion(this)
        uniffiCheckApiChecksums(this)
    }

    external fun ffi_my_prover_test_uniffi_contract_version(): Int
}

internal object UniffiLib {
    init {
        Native.register(UniffiLib::class.java, findLibraryName(componentName = "my_prover_test"))
    }

    external fun uniffi_my_prover_test_fn_func_prove(): Long
}

public fun uniffiEnsureInitialized() {
    IntegrityCheckingUniffiLib
    UniffiLib
}
"#;

    #[test]
    fn sentinel_matches_pinned_uniffi_codegen() {
        let patched = inject_prover_load_library(CURRENT_UNIFFI_FIXTURE).unwrap();
        assert_eq!(patched, EXPECTED_INJECTED_FIXTURE);
    }

    #[test]
    fn injection_fails_when_object_renamed() {
        let drifted = CURRENT_UNIFFI_FIXTURE.replace(
            "object IntegrityCheckingUniffiLib",
            "object IntegrityCheckUniffi",
        );
        let err = inject_prover_load_library(&drifted).unwrap_err();

        assert!(
            err.contains("matched 0 times"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn injection_fails_when_init_block_dropped() {
        let drifted = CURRENT_UNIFFI_FIXTURE.replace(
            "    init {\n        Native.register(IntegrityCheckingUniffiLib::class.java",
            "    fun setup() {\n        Native.register(IntegrityCheckingUniffiLib::class.java",
        );
        let err = inject_prover_load_library(&drifted).unwrap_err();

        assert!(
            err.contains("matched 0 times"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn injection_fails_when_anchor_duplicates() {
        let drifted = format!("{CURRENT_UNIFFI_FIXTURE}\n{CURRENT_UNIFFI_FIXTURE}");
        let err = inject_prover_load_library(&drifted).unwrap_err();

        assert!(
            err.contains("matched 2 times"),
            "unexpected error message: {err}"
        );
    }
}
