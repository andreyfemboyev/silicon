# Security

Architecture files select typed execution operations from a closed enumeration. Guest code operates on virtual registers, byte memory, and a bounded virtual console. The machine exposes no guest system calls. File dialogs mediate desktop filesystem reads and writes. The production UI is bundled and the desktop does not include account, telemetry, shell, or network-command integrations.

JSON imports are size-bounded and parsed as typed records. Validation enforces instruction field, memory, circuit, and simulation limits before execution. The dispatcher stops invalid guest instructions through structured errors. A forged project can still consume CPU or memory within configured limits. The application and system webview are ordinary host processes. Use platform sandboxing if an additional containment boundary is required.

For a public deployment, maintainers should enable GitHub private vulnerability reporting. Report vulnerabilities privately through the repository's Security tab once it is available. Include the affected release, the relevant subsystem, and a minimal reproduction that avoids personal data.
