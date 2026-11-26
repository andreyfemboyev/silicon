# Building SILICON v0.1.0-alpha

Use a current stable Rust toolchain and the checked-in Cargo lockfile. Use Node.js 22 or later for the desktop frontend. Dependency installation and release tooling require network access on a fresh build machine. Installed applications use bundled assets and run offline.

## CLI

```sh
cargo build --locked --release -p silicon
cargo test --locked
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
```

The output is `target/release/silicon` on Linux and `target/release/silicon.exe` on Windows. For a static Linux x86-64 build:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --locked -p silicon --release --target x86_64-unknown-linux-musl
```

The static output is `target/x86_64-unknown-linux-musl/release/silicon`. To cross-build the Windows CLI on Debian with MinGW-w64 installed:

```sh
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc-posix \
  cargo build --locked -p silicon --release --target x86_64-pc-windows-gnu
```

The cross-built CLI is `target/x86_64-pc-windows-gnu/release/silicon.exe`.

## Linux desktop

On Ubuntu 24.04 or Debian with WebKitGTK 4.1, install native build dependencies:

```sh
sudo apt-get update
sudo apt-get install build-essential curl file libssl-dev libwebkit2gtk-4.1-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
npm ci --prefix desktop/ui
npm --prefix desktop/ui run build
cargo test --locked -p silicon-desktop --features custom-protocol
cd desktop
./ui/node_modules/.bin/tauri dev
```

Build Debian and AppImage packages from the `desktop` directory:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./ui/node_modules/.bin/tauri build --ci --bundles deb,appimage
```

Look in `target/release/bundle/deb/` and `target/release/bundle/appimage/` from the repository root. AppImage creation uses Tauri's Linux deployment tools and may need a host with a compatible FUSE or extraction environment. The DEB uses the distribution WebKitGTK and GTK runtime packages.

## Windows desktop

Install the MSVC Rust toolchain, Visual Studio C++ Build Tools and Windows SDK, and Node.js. Run the following in PowerShell:

```powershell
npm ci --prefix desktop/ui
npm --prefix desktop/ui run build
cargo test --locked -p silicon-desktop --features custom-protocol
Set-Location desktop
.\ui\node_modules\.bin\tauri.cmd build --ci --bundles nsis
```

The installer is written to `target/release/bundle/nsis/`. The bundle configuration requests a WebView2 offline installer for machines missing the runtime. The initial NSIS build needs network access to acquire packaging tools and the WebView2 installer. The binaries and installers are unsigned.

## Frontend checks

```sh
cargo build --locked -p silicon
npm ci --prefix desktop/ui
npm --prefix desktop/ui run build
cd desktop/ui
npx playwright install chromium
npm test
```

Set `SILICON_CHROMIUM` to test with an existing Chromium executable. Set `SILICON_CLI` to use a CLI binary at a nondefault path. The Playwright test uses responses from the CLI for reference signal and machine data, with a test double for the Tauri transport. The desktop package separately tests the registered Tauri IPC handler.


For a native smoke test on Linux, run `tauri-driver` with the local WebKitWebDriver available, then use:

```sh
python3 desktop/tests/native_smoke.py /absolute/path/to/silicon-desktop
```

On an isolated X11 display with `xdotool`, add `--gtk-dialogs` to exercise native save and reopen. Set `APPIMAGE_EXTRACT_AND_RUN=1` for an AppImage test that does not require a FUSE mount.
