export type Component = {
  kind: string;
  value?: string;
  period?: number;
  words?: number[];
  address_bits?: number;
  lsb?: number;
  ports?: number[];
  output?: number;
  circuit?: Circuit;
};
export type Node = {
  name: string;
  width: number;
  component: Component;
  inputs: number[];
  x?: number;
  y?: number;
};
export type Circuit = { version: number; nodes: Node[] };
export type Project = {
  version: number;
  name: string;
  architecture: { name: string; [key: string]: unknown };
  source: string;
  circuit: Circuit;
};
export type Wave = {
  ticks: number;
  events: { tick: number; node: number; value: string }[];
  conflicts: [number, string][];
};
export type Assembly = {
  binary: number[];
  symbols: Record<string, number>;
  source_map: Record<string, number>;
};
export type Snapshot = {
  state: {
    pc: number;
    sp: number | null;
    registers: number[];
    cycles: number;
    halted: boolean;
    flags: Record<string, boolean>;
  };
  trace: {
    cycle: number;
    pc: number;
    instruction: string;
    reads: number[];
    writes: number[];
  }[];
  memory: number[];
  memory_base: number;
  stack: number[];
  console: string;
  stop: string | null;
  reversed: boolean;
};
export type T = (english: string, russian: string) => string;
export const hex = (value: number, digits = 4) =>
  value.toString(16).toUpperCase().padStart(digits, "0");
export const position = (node: Node, index: number) => ({
  x: node.x ?? 30 + (index % 4) * 165,
  y: node.y ?? 30 + Math.floor(index / 4) * 100,
});
export const library = [
  "input",
  "clock",
  "and",
  "or",
  "xor",
  "not",
  "nand",
  "nor",
  "xnor",
  "mux",
  "demux",
  "add",
  "carry",
  "full_add",
  "full_carry",
  "equal",
  "register",
  "counter",
  "rom",
  "ram",
  "resolve",
  "output",
  "slice",
  "subcircuit",
];
export const ports = (component: Component) => {
  switch (component.kind) {
    case "input":
    case "clock":
      return [];
    case "counter":
      return ["CLK"];
    case "not":
    case "output":
    case "slice":
      return ["IN"];
    case "rom":
      return ["ADDR"];
    case "register":
      return ["D", "CLK"];
    case "demux":
      return ["D", "SEL"];
    case "mux":
      return ["A", "B", "SEL"];
    case "full_add":
    case "full_carry":
      return ["A", "B", "CIN"];
    case "ram":
      return ["ADDR", "DATA", "WE", "CLK"];
    case "subcircuit":
      return (component.ports ?? []).map(
        (p, i) => component.circuit?.nodes[p]?.name ?? `IN ${i}`,
      );
    default:
      return ["A", "B"];
  }
};
