# v0.1.0-alpha limitations

- Architecture definitions use a fixed instruction width per ISA, one byte-addressed RAM space, immediate and register-indirect memory addressing, and a single optional write-only console port. Variable-length instructions, custom status register layouts and per-flag rules, control microsequences, mapped memory regions, and interrupts are pending.
- The MICRO-8 circuit CPU implements SET, ADD, and HALT. Its supplied ROM is initialized from the matching reference program. The rest of the MICRO-8 ISA, automatic assembly-to-ROM linking, and circuit CPUs for SILICON-16 and SILICON-32 are pending.
- The schematic editor has resizable side panes and JSON-editable subcircuit instances. Recursive graphical editing, a shared named module library, multi-output symbol pins, automatic wire routing, and multi-selection are pending.
- The waveform view captures settled values at integer ticks. Triggers seek a rising or falling edge within the capture. Trigger-controlled acquisition, internal hierarchy browsing, and propagation-delay or glitch modeling are pending. The waveform canvas renders the visible portion of all top-level channels.
- Reverse history is bounded and lives in memory. Persisted checkpoints, cycle-accurate instruction timing, and hardware co-simulation are pending. The desktop shows a bounded disassembly preview and captures the current binary at load time. Self-modifying code is reflected by actual step trace and memory views.
- English and Russian cover UI controls. Backend diagnostics and low-level opcode/component identifiers are English.
- Desktop runtime depends on the platform system webview. AppImage and NSIS verification status for this delivery is listed in [Verification](VERIFICATION.md).
