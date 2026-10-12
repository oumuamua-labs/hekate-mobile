# Hekate Mobile

[![License: AGPL-3.0-only](https://img.shields.io/badge/License-AGPL--3.0--only-blue.svg)](./LICENSE)

*Copyright (c) 2026 Andrei Kochergin and Oumuamua Labs.*

Prove in zero knowledge on an iPhone or an Android phone. You write the statement once in Rust, and two build
commands turn that crate into two xcframeworks and an `.aar` with the Hekate prover inside. The app team calls one
`async` function in Swift or one `suspend` function in Kotlin and gets a proof back, and `Task.cancel()` or
`Job.cancel()` stops the prover.

Swift and Kotlin guides: [oumuamua.dev/mobile](https://oumuamua.dev/mobile). The proof system:
[Hekate](https://github.com/oumuamua-labs/hekate).

> [!NOTE]  
> Hekate's open code is AGPL-3.0-only; its signed prover binary shipped as free binaries for macOS (Apple Silicon),
> Linux (ARM64, glibc), iOS (ARM64, device and simulator), and Android (ARM64).
> A product shipped under other terms, open source or closed, needs a commercial license.

---

## Measured On iPhones

On-device proving, measured on real phones. These are the two example proofs with zero knowledge on:

| Proof                                | Phone                         |  Prove | Verify |         Proof size |
|:-------------------------------------|:------------------------------|-------:|-------:|-------------------:|
| ML-DSA-65, key in the Secure Enclave | iPhone 14 Pro Max, iOS 26.7.1 | 1.34 s |  26 ms |          1,577 KiB |
| RSA-2048 PKCS#1 v1.5                 | iPhone 14 Pro Max, iOS 26.7.1 | 485 ms |  21 ms | 1,808 to 1,814 KiB |
| RSA-2048 PKCS#1 v1.5                 | iPhone 11 Pro Max, iOS 18.6.2 | 832 ms |  34 ms | 1,811 to 1,815 KiB |

Each figure is the best of three proofs with the phone on battery, the signed `ct` prover 0.15.0 and hekate
0.37.0. The apps in [examples/](examples/README.md) signed with keys the phones made and reported these numbers.
That README says what each row of the report means and how to run the apps on your own phone.

---

## What You Can Do

The work splits into three roles, and each has one job:

- On the Rust side, describe the statement in `src/program.rs` and its inputs and outputs in `src/inputs.rs`.
  You get two xcframeworks, Swift bindings and one `.aar`, ready to hand over.
- On the iOS side, embed the frameworks, add the bindings file and declare `MyProver`. You get
  `try await MyProver.prove(inputs:)`, stopped by `Task.cancel()`.
- On the Android side, add the `.aar` and JNA and declare `MyProver`. You get `MyProver.prove(inputs)` in a
  coroutine, stopped by `Job.cancel()`.

The API the app calls is made of your own types: `MyInputs`, `MyOutput`, `CancelToken` and a typed error. No field
element, trace or transcript appears in Swift or Kotlin. `MyProver` is a type the app declares once in a few lines;
[swift/README.md](swift/README.md) and [kotlin/README.md](kotlin/README.md) show it, with the remaining setup on
each platform.

---

## Quick Start

Five commands generate a crate and build it for both platforms:

```bash
cargo generate --git https://github.com/oumuamua-labs/hekate-mobile template --name my-prover
cd my-prover
cargo xtask doctor
cargo xtask ios-build --unsigned
cargo xtask android-build
```

The first command needs [`cargo-generate`](https://crates.io/crates/cargo-generate) 0.21 or newer. `doctor` names
every missing prerequisite and prints the fix. It counts a missing codesigning identity as a failure, which an
`--unsigned` build can ignore.

The generated crate proves a sample statement as it stands: knowledge of a 32-bit value pinned to a public input.
Replace it with yours in `src/program.rs` and `src/inputs.rs`.

`cargo generate` asks for the prover mode:

- `ct` is the default. Choose it when the witness holds anything private: keys, credentials, user data. It costs
  constant-time arithmetic.
- `public` uses variable-time tables. Choose it when every input is public, or when you are measuring
  throughput. It is faster, and the running time leaks the witness.

---

## Examples

[examples/](examples/README.md) holds a complete crate with an iPhone app and an Android app around it. The apps
sign with a key the phone made, prove an RSA-2048 or an ML-DSA-65 signature, verify the proof and report the
time, the proof size and the peak memory.

---

## The Rust Side

You edit two files:

- `src/inputs.rs` holds `MyInputs` and `MyOutput`, the records the app passes in and gets back.
- `src/program.rs` holds the circuit, the witness builder and the public inputs.

`src/lib.rs` exports them to Swift and Kotlin as one function:

```rust
#[uniffi::export]
pub fn prove(inputs: MyInputs, cancel: Option<Arc<CancelToken>>) -> Result<MyOutput, ProveError>
```

That function already does the following:

- A panic in your code or in the prover comes back as `ProveError::Panic`, and the app keeps running.
- Every proof draws a fresh 32-byte seed from the operating system.
- `CancelToken::request()`, called from any thread, stops the prover, and `prove` returns `ProveError::Cancelled`.
- `MyOutput` carries the program id, as 64 hex characters, next to the proof bytes. Your verifier decodes it and
  passes it to `HekateVerifier::verify`, which rejects an id that differs from the one it computes from its own
  program.
- The witness trace is zeroized when it is dropped.

One thing it does not do: check your witness against the constraints. The prover proves the trace it is given,
and a witness that breaks a constraint comes back as a proof that no verifier accepts. Check the inputs in
`build_witness`, and run `hekate_sdk::preflight` on the program in your tests.

Set `TRANSCRIPT_LABEL` in `src/program.rs` to a string of your own. The verifier must open its transcript with the
same bytes.

---

## Commands

### `cargo xtask doctor [ios|android]`

Checks the machine before a build fails halfway. A failed check prints the fix, and the exit code is non-zero when
any check fails. Without an argument it checks both platforms.

- For iOS: Xcode with the iOS SDK and its license accepted, the Rust targets `aarch64-apple-ios` and
  `aarch64-apple-ios-sim`, a codesigning identity in the keychain and `HEKATE_IOS_TEAM_ID` in the environment.
- For Android: `ANDROID_NDK_HOME` pointing at an installed NDK, the Rust target `aarch64-linux-android`,
  `cargo-ndk` and JDK 17 or newer.

### `cargo xtask ios-build [--unsigned]`

Writes three things under `target/`, named after your crate (`my-prover` here), for iOS 16 and up:

- `xcframeworks/MyProver.xcframework` is your prover, for devices and Apple-silicon simulators.
- `xcframeworks/HekateProverCdylib.xcframework` is the Hekate prover, with the same two slices.
- `swift/MyProver/my_prover.swift` is the Swift bindings file the app compiles.

The app embeds both frameworks: yours loads the prover's by `@rpath`. Signing uses the certificate that matches
`HEKATE_IOS_TEAM_ID`, your 10-character Apple Developer Team ID. Without that variable the build stops; pass
`--unsigned` for CI and local runs.

[swift/README.md](swift/README.md) is the iOS developer's half.

### `cargo xtask android-build`

Writes `target/aar/my-prover-release.aar`: your library, the Hekate prover library and the generated Kotlin
bindings in one archive, for `arm64-v8a` and Android API 26 and up. Both libraries need the ARMv8 cryptography
extension, and both load on devices with 16 KB pages.

The bindings load the prover library before your own. That patch is pinned to the Kotlin output of `uniffi`
0.32.2: after a `uniffi` bump the build stops with a message that names the anchor to update in
`xtask/src/android_build.rs`.

[kotlin/README.md](kotlin/README.md) is the Android developer's half.

---

## Versions

`Cargo.toml` pins every Hekate crate to one exact release and `uniffi` to `0.32.2`. To move to a new Hekate
release, change those pins together.

`hekate-prover-sys` fetches the prover binary for each target during `cargo build` and checks its SHA-256, its
Ed25519 signature and its ML-DSA-65 signature against keys pinned in the crate. A mismatch fails the build.
Binaries exist for iOS devices, the Apple-silicon iOS simulator and Android arm64, in both modes.

Nothing on the device checks those signatures again. The binary ships inside your app and is covered by your own
Apple and Play signing.

---

## Security

> [!WARNING]
> Hekate Mobile has not been independently audited and may contain bugs and security flaws.
> USE AT YOUR OWN RISK!

[Soundness and Security](https://oumuamua.dev/hekate/docs/advanced/soundness-and-security) collects the guarantees
a Hekate proof gives today and the checks that stay with you. Report a vulnerability by mail to
<info@oumuamua.dev> and keep it out of public issues.

---

## License

AGPL-3.0-only. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

Linking against the proprietary prover shared library is covered by the
[prover linking exception](LICENSE-EXCEPTION), an additional permission under AGPL section 7.

Commercial licenses are available from Oumuamua Labs <info@oumuamua.dev>. The Free, Pro and Enterprise tiers
are compared at [oumuamua.dev/hekate/pricing](https://oumuamua.dev/hekate/pricing).