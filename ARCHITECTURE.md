# Architecture

| Directory | Responsibility |
| --- | --- |
| `core/` | Versioned models, ISA validation, assembler and disassembler, machine, circuit engine and VCD |
| `cli/` | Clap commands, bounded file reads, reports and local artifact export |
| `desktop/` | Tauri application, native file dialogs, command handlers and debugger state |
| `desktop/ui/` | React workbench, locally bundled Monaco, Canvas 2D schematic and analyzer |
| `examples/` | Three original ISAs, source listings and the MICRO-8 circuit CPU |

The assembler and disassembler share the architecture's opcode masks and field layout. The instruction engine decodes the same format and dispatches a typed Rust operation for each instruction. It owns a bounded guest memory, registers, status flags, PC, optional SP, and an optional virtual console address. The desktop invokes Rust commands for computation. It keeps source and schematic edits in the UI and receives bounded debugger views from Rust.

The circuit engine compiles port dependencies and evaluates four-state buses. Register, counter, and RAM outputs form state boundaries. Nested subcircuits are expanded before validation. The signal analyzer consumes the engine's event records, including tick-zero values. The engines share no automatic translation between instruction semantics and digital components.

The reference MICRO-8 circuit fetches its own ROM word, extracts instruction fields, decodes SET, ADD and HALT, and clocks its own PC, registers and flags. A test assembles the source, executes it in the machine, and compares both engines at every tick in the fixture. Circuit timing is in simulation ticks. The instruction count advances by one per retired machine instruction.
