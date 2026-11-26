# Contributing

Build the CLI and run the default workspace tests before sending a patch. For frontend or desktop work, also run the TypeScript build, Playwright integration flow, and Tauri IPC tests described in [Building](BUILDING.md).

Preserve determinism when changing execution or circuit evaluation. Include regression coverage for altered encoding, state commits, capture events, or validation rules. Update the format documentation when adding a field and preserve the behavior of existing version 1 projects. Include invocation, hardware, and build configuration with benchmark results.
