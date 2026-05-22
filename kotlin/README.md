# Hekate Kotlin

Async / cancellation facade over the UniFFI-generated Kotlin bindings for Hekate Android. Maps Kotlin coroutines
(`suspend`, `Job`, structured cancellation) onto the synchronous `prove(inputs, cancel)` C-ABI call the prover
crate exports through UniFFI.

The wrapper is one suspending extension function. Its job is two things and nothing more: hand the multi-second
prove to a dispatcher so it does not block the calling thread, and route coroutine cancellation through the FFI
`CancelToken` so a structured `Job.cancel()` actually reaches Rust.

---

## Installation

The `.aar` is produced by `cargo xtask android-build` (see the root README) and lives at
`target/aar/<pkg>-release.aar`. Consume it one of two ways:

**JitPack** (tag-driven, no local publish step):

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        maven { url = uri("https://jitpack.io") }
    }
}

// app/build.gradle.kts
dependencies {
    implementation("com.github.oumuamua-labs:hekate-mobile:0.1.0")
}
```

**Local Maven** (for in-tree development, pre-tag iteration, or air-gapped CI):

```bash
./gradlew :lib:publishToMavenLocal
```

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        mavenLocal()
    }
}

// app/build.gradle.kts
dependencies {
    implementation("dev.oumuamua.hekate:hekate:0.1.0")
}
```

The `.aar` already carries both `lib<dev>.so` and `libhekate_prover_cdylib.so` under `jniLibs/<abi>/`. No
additional native artifact needs to ride alongside.

---

## Usage

```kotlin
import dev.oumuamua.hekate.prove
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

viewModelScope.launch {
    val output = MyProver.prove(inputs, Dispatchers.Default)
}
```

`Job.cancel()` on the enclosing coroutine — including parent-driven structured cancellation, `ViewModel`
teardown, or an explicit `cancel()` from a `CoroutineScope` — automatically calls `CancelToken.request()`
through `suspendCancellableCoroutine.invokeOnCancellation`. Rust's prover loop polls the token at instruction
boundaries and unwinds with `ProveError.Cancelled`. There is no manual cancel-token plumbing on the call site.

The `dispatcher` parameter is configurable for a reason: `Dispatchers.Default` is sized to CPU cores and is
shared with every other CPU-bound coroutine in the process. A multi-second prove pinned to `Default` starves
that pool when several proofs run concurrently or alongside other CPU work. For batch proving or background
flows, pass `Dispatchers.IO` (elastic) or a dedicated `newFixedThreadPoolContext("prove", n)` sized to the
prover's parallelism budget.