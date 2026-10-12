# Hekate Mobile Examples

Two apps, one for iPhone and one for Android, that show what a zero-knowledge proof costs on a phone. Each app
signs a message with a key the phone made, proves that the signature is valid without revealing it, verifies the
proof and reports the time, the proof size and the peak memory.

- `prover/` is the Rust crate both apps call. The template's `cargo xtask` builds it.
- `ios/` is the iPhone app. It signs with the Security framework and CryptoKit.
- `android/` is the Android app. It signs with the Android Keystore and Conscrypt.

The apps prove two statements. The RSA-2048 one runs on iOS 16 and Android 12. The ML-DSA-65 one needs a
platform that signs ML-DSA: iOS 26, built with Xcode 26, or Android 17.

---

## Run It

### iPhone

Three commands build the frameworks and open the project:

```bash
cd examples/prover
cargo xtask ios-build --unsigned
open ../ios/SignatureExample.xcodeproj
```

In Xcode, pick your team under *Signing & Capabilities* of the `SignatureExample` target, select your phone and
run. A team that cannot register `dev.oumuamua.hekate.examples.signatures` needs another bundle identifier.

`--unsigned` is enough: the project has Xcode sign the two embedded frameworks when it builds the app.
`cargo xtask doctor ios` lists what the build needs. It counts a missing codesigning identity as a failure,
which an unsigned build can ignore.

### Android

Four commands build the `.aar` and install the app:

```bash
cd examples/prover
cargo xtask android-build
cd ../android
./gradlew :app:installDebug
```

The build needs `ANDROID_NDK_HOME`, `cargo-ndk` and JDK 17, and `cargo xtask doctor android` checks them. Gradle
finds the SDK through `ANDROID_HOME`. The app uses the Kotlin package from `kotlin/` in this repository and the
`.aar` from `prover/target/aar/`.

### In the app

Choose a statement and tap *Sign, prove and verify*. The app creates the key on first use, signs three fresh
32-byte challenges, proves and verifies each one, and lists the result. *Cancel* stops a run. *Share the report*
exports the result as text.

Where the platform cannot sign ML-DSA-65, the app says why when you choose that statement. An iPhone app built
with Xcode 16 is such a case: its SDK has no ML-DSA.

---

## What Each Proof States

### RSA-2048

The proof states that its maker knows a message and an RSA-2048 signature on it that verifies under the modulus
`N`. `N` is public. The message and the signature stay hidden. The scheme is PKCS#1 v1.5 with SHA-256, proved by
`hekate-rsa`.

Two things this statement does not give you:

- It does not show that the maker holds the private key. Any signature the key ever released will do.
- The message is hidden, and a verifier cannot tie the proof to a challenge it chose. A login needs more than
  this statement.

The message is at most 55 bytes, one SHA-256 block, and the public exponent must be 65537.

### ML-DSA-65

The proof states that an ML-DSA-65 signature on a public message verifies under a public key whose hash is `tr`.
The message and `tr` are public. The public key and the signature stay hidden. The verification is FIPS 204,
proved by `hekate-pqc`.

- The message is public, which lets a verifier choose it. A proof over a fresh message shows that the key behind
  `tr` signed that message after the verifier chose it.
- `tr` is the 64-byte SHAKE-256 hash of the public key, the same in every proof from one key. Those proofs are
  linkable.
- The message is 32 bytes, signed under the empty context.

### Proof size

A proof is 1,806 to 1,816 KiB for RSA-2048 and 1,572 to 1,580 KiB for ML-DSA-65. For scale, an ML-DSA-65
signature is 3,309 bytes.

Measured on the signed `ct` cdylib 0.15.0 with hekate 0.37.0 and `Config::default()`, zero knowledge on, on an
Apple M3 Max, in the iOS simulator and the Android emulator.

---

## What The Report Shows

Each time is the best of three runs, with the worst next to it.

- Witness is the time Rust takes to build the execution trace from the signature, before the prover starts.
- Prove is the time inside the prover. Hekate's published prove figures cover this phase alone: compare them
  with this row.
- Whole call is what your app waits for: the `prove` call with its input checks, both phases and the FFI.
- Verify is the time inside `HekateVerifier::verify`.
- Proof is the size of the serialized proof. It differs by a few KiB from run to run.
- Peak memory is the highest value since launch. iOS reports the physical footprint
  (`ledger_phys_footprint_peak`) and Android the resident set (`VmHWM`). These are two different measures: do
  not compare one platform's figure with the other's.

The remaining rows name the conditions: where the key lives, the device, the system, the Hekate and prover
versions, the prover mode, the thermal state before and after, power saving and power.

A simulator or an emulator runs on the computer's processor. Its timings say nothing about a phone, and the
report marks them.

For a number worth publishing, launch the app from the Home screen or the launcher, and launch it fresh for each
measurement. A run started from Xcode has a debugger attached, and the peak memory covers the whole life of the
process. On iOS the proof runs at `TaskPriority.userInitiated`, on Android on `Dispatchers.Default`.

### Cancelling

*Cancel* stops the prover at its next poll. It cancels the Swift task or the coroutine job, and the package
requests the cancel token. The witness build takes no token: a cancel that arrives during it takes effect when
the build ends. Both prove functions check the token on entry and again after the witness build.

---

## Where The Keys Live

No private key reaches Rust. The witness is the signature, and the report names where the key was.

On an iPhone the RSA key is a permanent Keychain item made by `SecKeyCreateRandomKey`; the Secure Enclave does
not hold RSA keys. The ML-DSA key is made by `SecureEnclave.MLDSA65` and stays in the Secure Enclave, and the
Keychain keeps its opaque blob. The iOS 27.0 simulator on an Apple M3 Max reports a Secure Enclave, and the app
uses it there too. Where `SecureEnclave.isAvailable` is false, CryptoKit makes a software key and the Keychain
keeps its seed.

On Android the RSA key is in the Android Keystore, and the report gives its security level. The ML-DSA key is in
the Keystore too when the device can make it. When it cannot, as on an emulator whose KeyMint has no ML-DSA, the
app signs with a Conscrypt key that lives in memory for the session.

---

## The Rust Crate

`prover/` is a crate from the template with the sample statement replaced by two real ones. Its `xtask` and its
Gradle wrapper are symbolic links into `template/`: the example is built by the commands a generated crate gets.

```rust
pub fn prove_rsa(inputs: RsaInputs, cancel: Option<Arc<CancelToken>>) -> Result<Proof, ProveError>

pub fn verify_rsa(
    program_id: String,
    proof_bytes: Vec<u8>,
    modulus: Vec<u8>,
) -> Result<Duration, VerifyError>

pub fn prove_mldsa(inputs: MlDsaInputs, cancel: Option<Arc<CancelToken>>) -> Result<Proof, ProveError>

pub fn verify_mldsa(
    program_id: String,
    proof_bytes: Vec<u8>,
    message: Vec<u8>,
    key_hash: Vec<u8>,
) -> Result<Duration, VerifyError>

pub fn mldsa_key_hash(public_key: Vec<u8>) -> Vec<u8>
```

A `Proof` carries the program id, the proof bytes, the witness time and the prove time. A verify function
returns the time the verifier took. Inputs are raw bytes: 256 big-endian bytes for the RSA modulus and
signature, 1,952 for the ML-DSA-65 public key, 3,309 for its signature and 64 for `tr`.

Two behaviours to know before you copy this crate:

- The prover does not check a witness. Handed a signature that does not verify, it would return a proof that no
  verifier accepts. Both prove functions refuse such a signature with `ProveError.Witness` instead.
- `HekateVerifier::verify` reports some bad proofs as `Ok(false)` and others as an error, a wrong public input
  among them. The verify functions turn both into `VerifyError.Rejected` with the verifier's reason.

The apps verify on the phone with the same crate, which keeps the example self-contained. A deployment verifies
on a server and pins the program id it audited.

The crate links the constant-time prover. To measure the variable-time one, change `default = ["ct"]` to
`default = ["public"]` in `prover/Cargo.toml` and build again.

---

## Tests

The Rust tests run on the host and need no phone:

```bash
cd examples/prover
cargo test --release -p signature-prover --lib
```

Their RSA vector is a signature the Security framework made on a Mac. Their ML-DSA vector, in `prover/vectors/`,
is a signature CryptoKit made on a Mac.

Each app has three tests that drive it through its screen. One per statement expects three verified proofs, and
one taps *Cancel* during a run. The ML-DSA test is skipped where the app cannot sign ML-DSA. Build the
frameworks or the `.aar` first; the Android tests need a device or an emulator attached. The iOS command names a
simulator: replace `iPhone 17` with one your Xcode has, which `xcrun simctl list devices available` lists.

```bash
cd examples/ios
xcodebuild test -project SignatureExample.xcodeproj -scheme SignatureExample \
    -destination 'platform=iOS Simulator,name=iPhone 17'

cd examples/android
./gradlew :app:connectedDebugAndroidTest
```

---

## License

AGPL-3.0-only. See [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
Commercial licenses are available from Oumuamua Labs <info@oumuamua.dev>.

Linking the proprietary prover shared library into an AGPL work is covered by
[LICENSE-EXCEPTION](../LICENSE-EXCEPTION), an additional permission under AGPL section 7.