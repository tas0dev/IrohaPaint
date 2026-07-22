# Android build

IrohaPaint uses ViewKit's native Android backend and is packaged as a
`NativeActivity` application.

## Requirements

- Android SDK and platform tools
- Android NDK
- Rust target: `rustup target add aarch64-linux-android`
- cargo-apk: `cargo install cargo-apk --version 0.10.0 --locked`
- `ANDROID_HOME` pointing to the Android SDK

## Build

```text
cargo android-apk
```

The debug APK is written to `target/debug/apk/irohapaint.apk`.

For a release build:

```text
cargo android-apk-release
```

Install the debug APK on a connected device with:

```text
adb install -r target/debug/apk/irohapaint.apk
```
