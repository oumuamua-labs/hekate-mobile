# Hekate Mobile

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache2-yellow.svg)](./LICENSE)

Developer tooling that turns a Rust prover crate into a signed `.xcframework` and `.aar` carrying a typed Swift / Kotlin
surface. Mobile teams consume one `await` call per prove with native cancellation. Zero ZK terminology leaks across the
boundary.

> [!IMPORTANT]
> Polyglot, separate from the Hekate Cargo workspace. Ships a `cargo-generate` template, an `xtask` build
> orchestrator, and Android Gradle scaffolding. Iterates independently of the open core and closed prover's release
> cadence.

---

## Audience and Role Boundary

Two roles. Hard line between them.

**Rust dev.** Single human, the SDK author. Writes `impl Air<Block128>` + `impl Program<Block128>` + witness-gen +
public-input encoder + the `#[uniffi::export]` surface. Runs the cross-compile + bindgen + framework-assembly
pipeline. Ships `MyProver.xcframework` + `MyProver.aar` to the mobile team.

**Mobile dev.** Swift / Kotlin engineer. Drops the artifact into Xcode / Android Studio. Calls
`try await MyProver.prove(inputs:)`. Never sees `Air`, `ProgramWitness`, `ProgramInstance`, `Transcript`, `Block128`,
or any ZK terminology.

The Rust dev's `#[uniffi::export]` surface **is** the mobile SDK. This repo is the toolchain that builds it.

---

## Quick Start

Prepared machine, four commands:

```bash
cargo generate --git <hekate-mobile-url> template --name my-prover
cd my-prover
cargo xtask doctor                                 # verify host prereqs
# edit src/program.rs + src/inputs.rs              # replace the seam with your domain
cargo xtask ios-build                              # → target/xcframeworks/{HekateProverCdylib,MyProver}.xcframework
```

`cargo generate` prompts `prover_mode`:

| Mode     | When                                              | Trade-off                                                |
|:---------|:--------------------------------------------------|:---------------------------------------------------------|
| `ct`     | **Default.** Private mobile witnesses.            | Constant-time arithmetic. Safe under timing observation. |
| `public` | Public data, debug builds, throughput benchmarks. | Variable-time table-math. Faster. Leaks timing.          |

The dev's outer crate links `hekate-prover-sys`, which pulls the signed cdylib (Ed25519 + ML-DSA-65 over the payload)
from the Oumuamua CDN at `cargo build` time. The `xtask` then bundles that cdylib into the distribution artifact so
the dynamic loader resolves it on-device.

---

## The Canonical UniFFI Pattern

Lives at `template/src/lib.rs`. The Rust dev rewrites `program.rs` and `inputs.rs`; this shape stays.

```rust
#[derive(uniffi::Object)]
pub struct CancelToken {
    inner: hekate_prover_sys::CancelToken
}

#[uniffi::export]
impl CancelToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: hekate_prover_sys::CancelToken::new(),
        })
    }

    pub fn request(&self) { self.inner.request(); }
}

#[derive(uniffi::Record)]
pub struct MyInputs {
    /* domain fields */
}

#[derive(uniffi::Record)]
pub struct MyOutput {
    pub proof_bytes: Vec<u8>, /* public outputs */
}

#[derive(thiserror::Error, Debug, uniffi::Error)]
pub enum ProveError { /* mapped from prover */ }

#[uniffi::export]
pub fn prove(inputs: MyInputs, cancel: Option<Arc<CancelToken>>) -> Result<MyOutput, ProveError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (air, instance, witness) = build_program_state(&inputs)?;

        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed)?;

        let proof = hekate_prover_sys::prove(
            TRANSCRIPT_LABEL,
            &air,
            &instance,
            &witness,
            &Config::default(),
            seed,
            cancel,
        ).map_err(map_prover_error)?;

        Ok(MyOutput::from_parts(hekate_sdk::serialize_proof_bytes(&proof), &inputs))
    }));

    result.unwrap_or_else(|p| Err(ProveError::Panic(panic_message(p))))
}
```

---

## `cargo xtask`

```
cargo xtask doctor                 verify host prereqs
cargo xtask ios-build              cross-compile, wrap frameworks, bind Swift, assemble + codesign xcframeworks
cargo xtask ios-build --unsigned   CI / dev escape hatch; refuses to silently skip signing
cargo xtask android-build          cross-compile via cargo-ndk, bind Kotlin, bundle jniLibs, assemble .aar
```

### `doctor`

| Check                                                                              | Failure mode                                                     |
|:-----------------------------------------------------------------------------------|:-----------------------------------------------------------------|
| Xcode Command Line Tools (`xcode-select`)                                          | iOS build cannot find `lipo`, `install_name_tool`, `xcodebuild`. |
| iOS Rust targets (`aarch64-apple-ios{,-sim}`)                                      | Cross-compile fails before it starts.                            |
| Android NDK (`ANDROID_NDK_HOME` set + path exists)                                 | Android linker wiring impossible.                                |
| Android Rust target (`aarch64-linux-android`)                                      | Cross-compile fails before it starts.                            |
| `cargo-ndk`                                                                        | Android linker wiring impossible.                                |
| Mach-O tools (`lipo`, `install_name_tool`)                                         | Framework rpath / id rewrite impossible.                         |
| Codesign identity (`security find-identity -p codesigning`) + `HEKATE_IOS_TEAM_ID` | App Store rejects unsigned xcframeworks.                         |

Non-zero exit on any failure. One-line install hint per failed check.

### `ios-build`

Per slice (`aarch64-apple-ios`, `aarch64-apple-ios-sim`):

1. `cargo build --release --target <slice>` against the live `hekate-prover-sys` manifest. Pulls the signed cdylib
   for the matching triple from the CDN.
2. Wrap the prover cdylib as `HekateProverCdylib.framework/` — synthetic `Info.plist`, binary renamed to drop
   `lib`/`.dylib`, `install_name_tool -id @rpath/HekateProverCdylib.framework/HekateProverCdylib`.
3. Wrap the dev cdylib as `<PascalCased>.framework/` with the same treatment.
4. On the dev framework,
   `install_name_tool -change <build-machine prover path> @rpath/HekateProverCdylib.framework/HekateProverCdylib`
   to rewrite the `LC_LOAD_DYLIB` entry the macOS linker baked from the prover's own `LC_ID_DYLIB`. Old path read
   dynamically via `otool -L`.
5. Generate Swift bindings via the per-template `[[bin]] uniffi-bindgen` target, which calls
   `uniffi::uniffi_bindgen_main()`. Pins bindgen to the template's `uniffi` crate by construction.
6. Wire bindings into each dev framework: copy `<crate>FFI.h` → `Headers/`, rewrite UniFFI's plain `module X { ... }`
   modulemap as `framework module X { ... }`, place at `Modules/module.modulemap`. Apple framework conventions
   require the `framework` keyword.
7. `xcodebuild -create-xcframework` per framework → `target/xcframeworks/{HekateProverCdylib,<Dev>}.xcframework`.
8. `codesign --sign $HEKATE_IOS_TEAM_ID --timestamp` per xcframework. `--unsigned` skips with a warning; missing env
   var without `--unsigned` fails fast.

Two xcframeworks, not one nested. App Store processing accepts the flat layout; nested
`Foo.framework/Frameworks/Bar.framework` is structurally allowed but fragile under Apple's pipeline.

### `android-build`

Per ABI (`arm64-v8a` → `aarch64-linux-android`; additional ABIs slot into the same loop):

1. `cargo ndk -t <abi> build --release -p <pkg> --lib`. `cargo-ndk` wires the NDK linker for the target triple
   against the live `hekate-prover-sys` manifest; the build script pulls the matching signed prover cdylib from
   the CDN.
2. Resolve both shared objects: the dev crate's `lib<dev>.so` from
   `target/<triple>/release/`, and `libhekate_prover_cdylib.so` from the freshest
   `target/<triple>/release/build/hekate-prover-sys-*/out/` directory.
3. Copy both `.so` files into `android/lib/src/main/jniLibs/<abi>/`. Both must coexist in the same ABI directory
   — Android's dynamic loader resolves `libhekate_prover_cdylib.so` by name when the dev `.so` declares it as
   a `NEEDED` entry.
4. Generate Kotlin bindings via the per-template `[[bin]] uniffi-bindgen` target, pointed at the freshly built
   dev `.so` with `--library`. Output lands under `android/lib/src/main/kotlin/uniffi/<crate>/<crate>.kt`. Pinning
   bindgen to the template's `uniffi` crate keeps generator and runtime in lockstep by construction.
5. Inject `System.loadLibrary("hekate_prover_cdylib")` into the generated Kotlin via a regex-anchored
   single-shot replacement against `internal object IntegrityCheckingUniffiLib { init { Native.register(…)`. The
   anchor is pinned to uniffi 0.31.1's codegen — if uniffi is bumped, fixture tests in `android_build.rs` fail
   loudly and force a re-pin. The prover library loads *before* the JNA registration call, guaranteeing the dev
   cdylib's `dlopen` of `libhekate_prover_cdylib.so` resolves against an already-mapped image rather than
   triggering recursive lookup from a half-initialized state.
6. `./gradlew :lib:assembleRelease --console=plain` from `android/`. Gradle packs `jniLibs/<abi>/*.so` and the
   patched Kotlin into `android/lib/build/outputs/aar/lib-release.aar`.
7. Copy to `target/aar/<pkg>-release.aar` for symmetry with the iOS artifact path.

One `.aar` per crate, not one per ABI: AGP packs all ABIs into a single archive and the consuming app picks at
install time.

---

## Dependencies

- `hekate-prover-sys` — embedded `artifacts/manifest.toml` resolves the signed cdylib from the Oumuamua CDN
  (5 triples × `ct`/`public` variants, Ed25519 + ML-DSA-65 signed).
- `uniffi` — pinned to a known-tested version; the per-template `uniffi-bindgen` bin keeps generator and runtime in
  lockstep by construction.
- `hekate-core`, `-program`, `-sdk`, `hekate-math`. Exact versions in `template/Cargo.toml`.

The build-time signature check (verified in `hekate-prover-sys`'s `build.rs`) is load-bearing only at the dev's
`cargo build`. Once the cdylib lands inside the dev's xcframework / aar, Apple notarization / Play signing cover the
entire app bundle; original publisher signatures do not ride along.

---

## Security & Audits

> [!WARNING]
> This implementation is currently UNAUDITED.
>
> It is provided "AS IS" with ABSOLUTELY NO WARRANTY under the terms of the Apache 2.0 License. The authors assume
> zero liability for any damages arising from its use in production environments.

---

## License

Apache-2.0. See [LICENSE](./LICENSE) and [NOTICE](./NOTICE).