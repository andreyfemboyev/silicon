# Instruction set architecture format v1

An architecture can be a JSON file by itself or the `architecture` property of a project. The instruction engine uses byte addresses and zero-indexed registers.

| Property | Meaning |
| --- | --- |
| `version` | Format version, currently 1 |
| `name` | Architecture name |
| `data_bits` | Register and load/store size, 8, 16 or 32 bits |
| `address_bits` | Memory size is 2 raised to this width, 4 to 20 |
| `registers` | General-purpose register count, 1 to 32 |
| `instruction_bytes` | Fixed instruction length, 1 to 4 bytes |
| `endian` | `little` or `big` for instruction, data, and stack words |
| `entry` | Initial byte PC and program load base |
| `stack` | Null or initial exclusive top of a descending stack |
| `console` | Null or address at which guest stores emit their low byte |
| `instructions` | Between 1 and 256 encoding records |

A record contains `mnemonic`, unsigned 32-bit `opcode` and `mask`, ordered `fields`, `op`, and optional `update_flags`. Each field gives `lsb`, `bits`, and `kind` (`register`, `unsigned`, or `signed`). A word matches when `word & mask == opcode`. Fixed bits and operand fields completely cover the encoding. Intersecting opcode match sets, field overlap, out-of-range fields, and unknown operations are errors. Signed fields use two's complement and are accepted for the second operand of `imm`.

Operand letters in the table represent a register `R` and an unsigned value `U`.

| Operation | Operands | Semantics |
| --- | --- | --- |
| `halt`, `nop`, `ret` | — | Stop, advance, or return to the four-byte PC popped from the stack |
| `mov` | R, R | Copy source to destination |
| `imm` | R, U or signed | Set destination to immediate, truncated to the data width |
| `add`, `sub`, `and`, `or`, `xor`, `shl`, `shr` | R, R | Apply operation to destination and source, write destination |
| `cmp` | R, R | Subtract for flags and leave the destination register intact |
| `load`, `store` | R, U | Read into or write from register using an immediate byte address |
| `load_indirect`, `store_indirect` | R, R | Read or write using the second register as byte address |
| `jump`, `jz`, `jnz`, `jcarry` | U | Branch unconditionally or conditional on zero, nonzero, or carry |
| `push`, `pop` | R | Push or pop one data word |
| `call` | U | Push the four-byte next PC and branch |

Flag updates apply to operations that produce a data result, when enabled. Z detects zero, N detects the top result bit, and C records unsigned overflow on ADD or no borrow on SUB and CMP. Other result operations clear C. Shift amounts at or above the active result width produce zero. Memory accesses may be unaligned, and the engine diagnoses accesses outside guest memory. An instruction counts one cycle. A fault leaves the architectural state at the faulting instruction.

Assembly uses mnemonics case-insensitively and labels case-sensitively. Operands are comma-separated. Register tokens use `r0` notation. Decimal, hexadecimal `0x`, and binary `0b` literals may carry a leading minus sign. `label:` binds a byte address. `.equ name, literal` binds a constant. Forward references to both are resolved. `.byte` and `.word` emit comma-separated data values. `;` and `#` begin comments. Reported line numbers are one-based and column 1 identifies the source line. Disassembly emits `.byte` for invalid encodings or trailing bytes.

## Reference encodings

All three reference architectures use little-endian words and put a five-bit opcode at the top of the instruction. Register fields descend from the next available bit. An immediate fills the remaining low bits. For instructions without an immediate, unused low bits are fixed to zero. The JSON definitions list the authoritative opcode assignments.

| ISA | Data | Address | Registers | Instruction | Stack | Console |
| --- | --- | --- | --- | --- | --- | --- |
| MICRO-8 | 8 | 8 | 4 | 2 bytes | — | — |
| SILICON-16 | 16 | 14 | 8 | 3 bytes | 0x3FF0 | 0x3FFC |
| SILICON-32 | 32 | 16 | 16 | 4 bytes | 0xFFF0 | 0xFFFC |

SILICON-16 and SILICON-32 add PUSH, POP, CALL, and RET. In SILICON-32, load a full-width literal from memory with `.word` and LD.
