# SILICON — v0.1.0-alpha

**Design. Simulate. Execute.**

First public alpha of an offline-first architecture development studio for digital logic and custom CPUs.

## Highlights
- Deterministic four-state circuit simulator, schematic editing and waveform/VCD export.
- Declarative ISA definitions, assembler and disassembler, register/memory inspection, bounded execution, breakpoints, watchpoints and reverse stepping.
- MICRO-8, SILICON-16 and SILICON-32 reference projects, plus a MICRO-8 SET/ADD/HALT circuit CPU fixture checked against the instruction engine.
- Native Tauri 2 desktop with bundled Monaco, and independent Rust CLI.

## Downloads
- Linux x86-64: static CLI archive, DEB and AppImage.
- Windows x86-64: CLI ZIP and NSIS setup executable.
- SHA256SUMS.txt: verify downloads before installation.
- Source: GitHub's source archives for this tag.

See [building instructions](https://github.com/andreyfemboyev/silicon/blob/main/BUILDING.md), [verification](https://github.com/andreyfemboyev/silicon/blob/main/docs/VERIFICATION.md) and [alpha limitations](https://github.com/andreyfemboyev/silicon/blob/main/docs/LIMITATIONS.md).

## Scope
The circuit CPU covers a subset of MICRO-8, not the full reference ISA. Instruction-level and circuit-level simulation are distinct; SILICON is not cycle-accurate hardware emulation or an FPGA synthesis tool. Local Linux desktop smoke tests are documented. Hosted builds and automated tests do not imply a native Windows desktop smoke test. Binaries and installers are unsigned. MIT licensed.
