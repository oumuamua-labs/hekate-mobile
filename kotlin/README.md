# Hekate Kotlin

Run a Hekate proof from a coroutine with one `suspend` call, and stop it by cancelling the job.

```kotlin
import dev.oumuamua.hekate.prove

viewModelScope.launch {
    val output = MyProver.prove(MyInputs(42u))
}
```

The blocking prover call runs on a dispatcher you choose, and `Job.cancel()` reaches the Rust prover. Requires
Android API 26 and an `arm64-v8a` device.

---

## Install

Publish the package to your local Maven repository. The build compiles with JDK 17 and fetches that toolchain when
the installed JDK is another version.

```bash
cd kotlin
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
    implementation("dev.oumuamua.hekate:hekate:0.3.0")
}
```

`0.3.0` is the `version` in `kotlin/lib/build.gradle.kts`.

---

## Add Your Prover

The Rust developer runs `cargo xtask android-build` and hands you one file, `my-prover-release.aar`, named after
the Rust crate. It holds the generated Kotlin bindings, the developer's native library and the Hekate prover.

```kotlin
// app/build.gradle.kts
dependencies {
    implementation(files("libs/my-prover-release.aar"))
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
```

The bindings call native code through JNA, and a file dependency brings no dependencies of its own, which is why
JNA is declared next to it.

The bindings live in the package `uniffi.my_prover` and give you a function `prove(inputs, cancel)` and a
`CancelToken` class. Declare the prover object once to make them a `HekateProver`:

```kotlin
import dev.oumuamua.hekate.CancelToken
import dev.oumuamua.hekate.HekateProver
import uniffi.my_prover.MyInputs
import uniffi.my_prover.MyOutput

class ProverToken : CancelToken {
    val generated = uniffi.my_prover.CancelToken()

    override fun request() {
        generated.request()
    }
}

object MyProver : HekateProver<MyInputs, MyOutput, ProverToken> {
    override fun makeCancelToken() = ProverToken()

    override fun prove(inputs: MyInputs, cancel: ProverToken): MyOutput =
        uniffi.my_prover.prove(inputs, cancel.generated)
}
```

---

## Use

`prove` takes a dispatcher and defaults to `Dispatchers.Default`. The proof blocks one thread of that dispatcher
until it returns. `Default` has as many threads as the device has cores and every CPU-bound coroutine in the app
shares it; for several proofs at once pass `Dispatchers.IO` or a pool of your own.

Cancel the job that runs the proof in any way coroutines offer: a cancelled scope, a `viewModelScope` whose
`ViewModel` is cleared, an explicit `job.cancel()`. The package calls `request()` on the token and the prover stops
at its next poll. The coroutine then ends with `CancellationException`, as a cancelled coroutine does.

```kotlin
try {
    val output = MyProver.prove(inputs)
    // output.proofBytes, output.programId, output.publicValue
} catch (e: ProveException.Panic) {
    // Rust panicked and the app is still running
} catch (e: ProveException) {
    // InvalidInput, Witness or Prover
}
```

UniFFI gives the Rust `ProveError` enum the Kotlin name `ProveException`. Its cases and the fields of `MyOutput` are
whatever the Rust side declares; these are the template's. A panic in Rust arrives as `ProveException.Panic`
because the template catches it before it crosses into Kotlin.

---

## License

AGPL-3.0-only. See [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
Commercial licenses are available from Oumuamua Labs <info@oumuamua.dev>.

Linking the proprietary prover shared library into an AGPL work is covered by
[LICENSE-EXCEPTION](../LICENSE-EXCEPTION), an additional permission under AGPL section 7.