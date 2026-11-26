# SILICON v0.1.0-alpha verification

The local verification machine was Debian 13 x86-64. It used Rust 1.100.0, Node.js 24.22.3, Chromium 156, and WebKitGTK 2.54.3. The Linux application was built with Tauri 2 and exercised using WebDriver, an Xvfb display, and an isolated network namespace with only loopback enabled.

## Checks run

| Check | Result and coverage |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --locked --workspace --all-targets --features silicon-desktop/custom-protocol -- -D warnings` | Passed |
| `cargo test --locked --offline --workspace --features silicon-desktop/custom-protocol` | 15 passed. Eleven engine tests, two CLI integration tests, and two desktop tests. |
| `npm --prefix desktop/ui run build` | Passed TypeScript validation and production bundling |
| `SILICON_CHROMIUM=/usr/bin/chromium npm --prefix desktop/ui test` | One browser integration test passed. The test uses real CLI signal and execution data with an IPC test double. |
| `desktop/tests/native_smoke.py` against the extracted DEB | Passed native schematic, simulation, waveform rendering, step, reverse, run, and bundled Monaco assembly via actual Tauri IPC. The X11 dialog check also saved and reopened a project. |
| `desktop/tests/native_smoke.py` against the AppImage | Passed schematic, simulation, waveform rendering, step, reverse, run, and bundled Monaco assembly offline in AppImage extraction mode. |
| `silicon test --json` from the extracted Linux CLI archive | All three reference programs passed. The CLI executable is statically linked and ran with networking disabled. |
| `unzip -t` and PE import inspection of the Windows CLI | Archive and x86-64 PE verified. Execution on Windows is pending. |

The MICRO-8 test compares every register, PC, the halt latch, and Z/N/C at each tick against the instruction engine for the supplied SET/ADD/HALT program. The ALU test covers every pair of eight-bit inputs for ADD and SUB. Circuit tests cover all binary combinations of the four logic states for the seven primitive gates, simultaneous register commits, RAM, driver conflicts, hierarchical port connections, and loop rejection. CPU tests cover round trips, endian and signed fields, faults, replay, reverse execution, and breakpoints and watchpoints.

The Debian package was built with `tauri build --ci --bundles deb,appimage`. The container lacked an installed system WebKitGTK package, so the DEB runtime check used staged Debian WebKitGTK libraries and a private relocation of their helper search path. The distributed DEB and its binary were unchanged. The AppImage build used `tauri bundle --ci --bundles appimage` with a temporary configuration pointing to the staged WebKit helper files. On an ordinary Debian build host, the Tauri bundler discovers the installed WebKit files. Cross-platform build commands are in `BUILDING.md` in the source archive.

## Benchmarks

A run of `silicon benchmark --iterations 100 --json` using the release Linux musl CLI executed 6,200 instructions in 3,818,582 ns and produced 16,500 circuit events in 5,205,605 ns. This single container run includes machine creation in the CPU workload and simulation setup in the circuit workload. Repeat the command on the intended host to measure that host.

## Delivery

The files are in `/workspace/output`. Check them with `sha256sum -c SHA256SUMS.txt` from that directory. The installer and executables are unsigned. GitHub Actions definitions are included in the source archive. The remote workflows were not run as part of this delivery.

| Artifact | Status |
| --- | --- |
| `silicon-v0.1.0-alpha-source.tar.gz` | Source, examples, tests, and build and release workflows |
| `silicon-cli-v0.1.0-alpha-linux-x86_64.tar.gz` | Built and offline self-tested |
| `silicon-cli-v0.1.0-alpha-windows-x86_64.zip` | Cross-built for Windows from the GNU Rust target. Execution pending. |
| `SILICON_0.1.0-alpha_amd64.deb` | Built, metadata checked, and extracted desktop smoke-tested offline |
| `SILICON_0.1.0-alpha_amd64.AppImage` | Built and native smoke-tested offline in extraction mode |
| Windows NSIS | Release configuration and Windows CI job included. Installer build and Windows desktop verification pending. |
| `SHA256SUMS.txt` | Integrity manifest for the delivered files |
