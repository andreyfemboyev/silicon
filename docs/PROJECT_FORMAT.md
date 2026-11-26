# Project format v1

A project is UTF-8 JSON with `version`, `name`, `architecture`, `source`, and `circuit`. See [the ISA specification](ISA_SPECIFICATION.md) for the architecture. The desktop stores source, schematic coordinates, nodes, and connection order in one file. It restores editable drafts, including disconnected circuits. Use `silicon validate` to check the architecture, source, and circuit together. The debugger snapshot and captured waveforms remain transient. VCD is exported independently.

A circuit is `{"version":1,"nodes":[...]}`. Each node has a unique `name`, `width`, `component`, `inputs`, and optional `x` and `y` coordinates. Inputs are zero-based node indices in the enclosing circuit. Component objects have a `kind` string and small typed parameters.

| Kind | Ordered inputs / parameters |
| --- | --- |
| `input`, `clock` | No inputs. Parameters are `value` (four-state string) or `period` (ticks) |
| `and`, `or`, `xor`, `nand`, `nor`, `xnor`, `add` | Two equal-width sources |
| `not`, `output` | One same-width source |
| `slice` | Source and parameter `lsb` |
| `mux` | Zero selection, one selection, one-bit select |
| `demux` | Data and one-bit select |
| `carry`, `equal` | Two equal-width sources, one-bit output |
| `full_add`, `full_carry` | Two equal-width sources and a one-bit carry-in |
| `register` | Data and clock. Optional unsigned `initial` parameter |
| `counter` | One-bit clock |
| `rom` | Address and parameter `words` (unsigned values) |
| `ram` | Address, data, one-bit write enable, one-bit clock. Parameter `address_bits` |
| `resolve` | Two to 64 equally sized drivers |
| `subcircuit` | One source per listed port. Parameters `circuit`, `ports`, and `output` |

A subcircuit has an embedded circuit, an ordered `ports` array of internal input-node indices and one internal `output` node index. The same embedded circuit can be reused at multiple sites with separate simulation state. Nesting and port widths are validated after flattening. The editor lets you place an inverter module, then replace its embedded circuit and port mapping in the parameter JSON inspector. Disconnected persisted inputs use the sentinel index 4294967295 and are rejected for simulation.

The project size cap is 8 MiB. Source is capped at 1 MiB. Semantically invalid drafts can be saved and reopened for correction.
