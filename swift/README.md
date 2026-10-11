# Hekate Swift

Run a Hekate proof from Swift with one `await`, and stop it by cancelling the task.

```swift
import Hekate

let output = try await MyProver.prove(inputs: MyInputs(value: 42))
```

The proof runs in a detached task: the caller's actor stays free until the `await` returns, and `Task.cancel()`
reaches the Rust prover. Requires iOS 16 or macOS 13.

---

## Install

```swift
dependencies: [
    .package(url: "https://github.com/oumuamua-labs/hekate-mobile.git", from: "0.1.0"),
],
targets: [
    .target(
        name: "YourApp",
        dependencies: [
            .product(name: "Hekate", package: "hekate-mobile"),
        ]
    )
]
```

SwiftPM names a remote package after the last component of its URL, which makes the package `hekate-mobile`. In
Xcode, *File > Add Package Dependencies* takes the same URL.

---

## Add Your Prover

The Rust developer runs `cargo xtask ios-build` and hands you three files. The names follow the Rust crate; a crate
called `my-prover` gives these:

| File                             | Where it goes        |
|:---------------------------------|:---------------------|
| `MyProver.xcframework`           | App target, embedded |
| `HekateProverCdylib.xcframework` | App target, embedded |
| `my_prover.swift`                | App target sources   |

`my_prover.swift` imports a C module, `my_proverFFI`, whose module map sits inside the xcframework. Point the Swift
compiler at it in *Other Swift Flags*:

```
-Xcc -fmodule-map-file=<path to MyProver.xcframework>/ios-arm64/MyProver.framework/Modules/module.modulemap
```

The bindings give you a free function `prove(inputs:cancel:)` and a `CancelToken` class. Declare the prover type
once to make them a `HekateProver`:

```swift
import Hekate

extension CancelToken: CancellableToken {}

enum MyProver: HekateProver {
    static func makeCancelToken() -> CancelToken { CancelToken() }

    static func prove(inputs: MyInputs, cancel: CancelToken) throws -> MyOutput {
        try generatedProve(inputs, cancel)
    }
}

private let generatedProve: (MyInputs, CancelToken?) throws -> MyOutput = prove(inputs:cancel:)
```

Inside `MyProver` the bare name `prove` resolves to the static method. The constant on the last line holds the
generated function under another name.

---

## Use

`prove(inputs:priority:)` takes an optional `TaskPriority` for the detached task; leave it out for the default.
Each running proof occupies one thread of Swift's cooperative pool until it returns.

Cancel the task that awaits the proof in any way Swift offers: a cancelled `TaskGroup`, a SwiftUI `.task` whose
view goes away, an explicit `task.cancel()`. The package calls `request()` on the token, the prover stops at its
next poll, and the `await` throws `ProveError.Cancelled`.

```swift
do {
    let output = try await MyProver.prove(inputs: inputs)
    // output.proofBytes, output.programId, output.publicValue
} catch ProveError.Cancelled {
    // the task was cancelled
} catch ProveError.Panic(let message) {
    // Rust panicked and the app is still running
} catch {
    // ProveError.InvalidInput, .Witness or .Prover
}
```

`ProveError` and the fields of `MyOutput` are whatever the Rust side declares; these are the template's. A panic in
Rust arrives as `ProveError.Panic` because the template catches it before it crosses into Swift.

---

## License

AGPL-3.0-only. See [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
Commercial licenses are available from Oumuamua Labs <info@oumuamua.dev>.

Linking the proprietary prover shared library into an AGPL work is covered by
[LICENSE-EXCEPTION](../LICENSE-EXCEPTION), an additional permission under AGPL section 7.