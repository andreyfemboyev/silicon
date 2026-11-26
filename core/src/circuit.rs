use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_NODES: usize = 4096;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Input {
        value: String,
    },
    Clock {
        period: u32,
    },
    And,
    Or,
    Xor,
    Not,
    Nand,
    Nor,
    Xnor,
    Mux,
    Demux,
    Add,
    Carry,
    FullAdd,
    FullCarry,
    Equal,
    Register {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial: Option<u32>,
    },
    Slice {
        lsb: u8,
    },
    Subcircuit {
        circuit: Box<Circuit>,
        ports: Vec<usize>,
        output: usize,
    },
    Counter,
    Rom {
        words: Vec<u32>,
    },
    Ram {
        address_bits: u8,
    },
    Resolve,
    Output,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub name: String,
    pub width: u8,
    pub component: Kind,
    #[serde(default)]
    pub inputs: Vec<usize>,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Circuit {
    pub version: u8,
    pub nodes: Vec<Node>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Event {
    pub tick: u32,
    pub node: usize,
    pub value: String,
}
#[derive(Debug, Serialize)]
pub struct Waveform {
    pub ticks: u32,
    pub events: Vec<Event>,
    pub conflicts: Vec<(u32, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Signal {
    value: u32,
    known: u32,
    z: u32,
}
impl Signal {
    const X: Self = Self {
        value: 0,
        known: 0,
        z: 0,
    };
    fn known(value: u32, width: u8) -> Self {
        Self {
            value: value & bits(width),
            known: bits(width),
            z: 0,
        }
    }
    fn parse(s: &str, w: u8) -> Result<Self> {
        if s.len() != usize::from(w) {
            return Err("input literal width mismatch".into());
        }
        let mut signal = Self::X;
        for (i, c) in s.bytes().rev().enumerate() {
            let bit = 1 << i;
            match c {
                b'0' => signal.known |= bit,
                b'1' => {
                    signal.known |= bit;
                    signal.value |= bit;
                }
                b'X' | b'x' => {}
                b'Z' | b'z' => signal.z |= bit,
                _ => return Err("signals use 0, 1, x and z".into()),
            }
        }
        Ok(signal)
    }
    fn render(self, w: u8) -> String {
        (0..w)
            .rev()
            .map(|i| {
                let b = 1 << i;
                if self.known & b == 0 {
                    if self.z & b == 0 {
                        'x'
                    } else {
                        'z'
                    }
                } else if self.value & b == 0 {
                    '0'
                } else {
                    '1'
                }
            })
            .collect()
    }
}
fn bits(w: u8) -> u32 {
    if w == 32 {
        u32::MAX
    } else {
        (1 << w) - 1
    }
}
fn sequential(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Register { .. } | Kind::Counter | Kind::Ram { .. }
    )
}
struct Plan {
    order: Vec<usize>,
    ranks: Vec<usize>,
    dependents: Vec<Vec<usize>>,
}
impl Circuit {
    pub fn validate(&self) -> Result<()> {
        self.flatten()?.plan().map(|_| ())
    }
    fn flatten(&self) -> Result<Self> {
        let mut flat = Vec::new();
        self.expand("", &[], 0, &mut flat)?;
        Ok(Self {
            version: self.version,
            nodes: flat,
        })
    }

    fn expand(
        &self,
        prefix: &str,
        connections: &[(usize, usize)],
        depth: usize,
        flat: &mut Vec<Node>,
    ) -> Result<usize> {
        if self.version != 1 || depth > 8 || flat.len() + self.nodes.len() > MAX_NODES {
            return Err("unsupported circuit version or hierarchy exceeds limits".into());
        }
        let base = flat.len();
        let mut unique = BTreeSet::new();
        for node in &self.nodes {
            if node.name.is_empty() || !unique.insert(&node.name) {
                return Err(format!("missing or duplicate node in {prefix}"));
            }
            let mut item = node.clone();
            item.name = format!("{prefix}{}", node.name);
            if item.inputs.iter().any(|&i| i >= self.nodes.len()) {
                return Err(format!("invalid connection on {}", item.name));
            }
            for input in &mut item.inputs {
                *input += base;
            }
            if matches!(item.component, Kind::Subcircuit { .. }) {
                item.component = Kind::Output;
            }
            flat.push(item);
        }
        for &(port, source) in connections {
            let node = flat.get_mut(base + port).ok_or("invalid subcircuit port")?;
            node.component = Kind::Output;
            node.inputs = vec![source];
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if let Kind::Subcircuit {
                circuit,
                ports,
                output,
            } = &node.component
            {
                if ports.len() != node.inputs.len()
                    || ports.iter().copied().collect::<BTreeSet<_>>().len() != ports.len()
                    || *output >= circuit.nodes.len()
                    || ports.iter().any(|&j| {
                        !matches!(
                            circuit.nodes.get(j).map(|p| &p.component),
                            Some(Kind::Input { .. })
                        )
                    })
                {
                    return Err(format!(
                        "invalid subcircuit ports on {}",
                        flat[base + i].name
                    ));
                }
                let prefix = format!("{}.", flat[base + i].name);
                let bindings = ports
                    .iter()
                    .zip(&node.inputs)
                    .map(|(&p, &j)| (p, base + j))
                    .collect::<Vec<_>>();
                let child = circuit.expand(&prefix, &bindings, depth + 1, flat)?;
                flat[base + i].inputs = vec![child + output];
            }
        }
        Ok(base)
    }

    fn plan(&self) -> Result<Plan> {
        let n = self.nodes.len();
        if self.version != 1 || n > MAX_NODES {
            return Err("unsupported circuit version or too many nodes".into());
        }
        let mut names = BTreeSet::new();
        let mut degree = vec![0; n];
        let mut dependents = vec![Vec::new(); n];
        for node in &self.nodes {
            if !(1..=32).contains(&node.width) {
                return Err(format!("invalid width on {}", node.name));
            }
        }
        for (i, node) in self.nodes.iter().enumerate() {
            use Kind::*;
            let w = node.width;
            if node.name.is_empty()
                || node.name.len() > 80
                || !node
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
                || !names.insert(&node.name)
            {
                return Err(format!("invalid or duplicate node {}", node.name));
            }
            let widths = node
                .inputs
                .iter()
                .map(|&j| {
                    self.nodes
                        .get(j)
                        .map(|s| s.width)
                        .ok_or_else(|| format!("missing input on {}", node.name))
                })
                .collect::<Result<Vec<_>>>()?;
            let valid = match &node.component {
                Input { value } => {
                    Signal::parse(value, w)?;
                    widths.is_empty()
                }
                Clock { period } => w == 1 && *period >= 2 && widths.is_empty(),
                And | Or | Xor | Nand | Nor | Xnor | Add => widths == [w, w],
                FullAdd => widths == [w, w, 1],
                Not | Output => widths == [w],
                Mux => widths == [w, w, 1],
                Demux => widths == [w, 1],
                Register { initial } => widths == [w, 1] && initial.is_none_or(|v| v <= bits(w)),
                Slice { lsb } => {
                    widths.len() == 1 && u16::from(*lsb) + u16::from(w) <= u16::from(widths[0])
                }
                Subcircuit { .. } => false,
                Counter => widths == [1],
                Equal | Carry => w == 1 && widths.len() == 2 && widths[0] == widths[1],
                FullCarry => {
                    w == 1 && widths.len() == 3 && widths[0] == widths[1] && widths[2] == 1
                }
                Ram { address_bits } => {
                    (1..=12).contains(address_bits) && widths == [*address_bits, w, 1, 1]
                }
                Rom { words } => {
                    widths.len() == 1
                        && widths[0] <= 12
                        && !words.is_empty()
                        && words.len() <= 1usize << widths[0]
                        && words.iter().all(|v| *v <= bits(w))
                }
                Resolve => (2..=64).contains(&widths.len()) && widths.iter().all(|x| *x == w),
            };
            if !valid {
                return Err(format!(
                    "invalid port count, width or parameters on {}",
                    node.name
                ));
            }
            if !sequential(&node.component) {
                for &j in &node.inputs {
                    degree[i] += 1;
                    dependents[j].push(i);
                }
            }
        }
        let mut ready: BTreeSet<_> = degree
            .iter()
            .enumerate()
            .filter_map(|(i, d)| (*d == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(n);
        let mut ranks = vec![0; n];
        while let Some(i) = ready.pop_first() {
            ranks[i] = order.len();
            order.push(i);
            for &j in &dependents[i] {
                degree[j] -= 1;
                if degree[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        if order.len() != n {
            return Err("combinational loop detected".into());
        }
        Ok(Plan {
            order,
            ranks,
            dependents,
        })
    }
    pub fn simulate(&self, ticks: u32, event_limit: usize) -> Result<Waveform> {
        let flat = self.flatten()?;
        let mut wave = flat.simulate_flat(ticks, event_limit)?;
        wave.events.retain(|event| event.node < self.nodes.len());
        wave.conflicts.sort_unstable();
        wave.conflicts.dedup();
        Ok(wave)
    }
    fn simulate_flat(&self, ticks: u32, event_limit: usize) -> Result<Waveform> {
        let plan = self.plan()?;
        let n = self.nodes.len();
        if ticks == 0
            || ticks > 100_000
            || (ticks as usize) * n > 5_000_000
            || !(1..=1_000_000).contains(&event_limit)
        {
            return Err("simulation exceeds tick, work or event limits".into());
        }
        let mut values = vec![Signal::X; n];
        let mut clocks = vec![Signal::X; n];
        let mut storage = self
            .nodes
            .iter()
            .map(|node| match node.component {
                Kind::Register {
                    initial: Some(value),
                } => Signal::known(value, node.width),
                Kind::Counter => Signal::known(0, node.width),
                _ => Signal::X,
            })
            .collect::<Vec<_>>();
        let mut ram: BTreeMap<(usize, u32), Signal> = BTreeMap::new();
        let sources: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                (sequential(&node.component) || matches!(node.component, Kind::Clock { .. }))
                    .then_some(i)
            })
            .collect();
        let mut wave = Waveform {
            ticks,
            events: Vec::new(),
            conflicts: Vec::new(),
        };
        for tick in 0..ticks {
            let previous = values.clone();
            let mut pending = if tick == 0 {
                (0..n).collect()
            } else {
                sources
                    .iter()
                    .map(|&i| plan.ranks[i])
                    .collect::<BTreeSet<_>>()
            };
            self.settle(&plan, &mut pending, &mut values, &storage, tick, &mut wave)?;
            for &i in &sources {
                let node = &self.nodes[i];
                use Kind::*;
                let clock_port = match node.component {
                    Register { .. } => 1,
                    Counter => 0,
                    Ram { .. } => 3,
                    _ => continue,
                };
                let clock = values[node.inputs[clock_port]];
                let rising = clocks[i] == Signal::known(0, 1) && clock == Signal::known(1, 1);
                clocks[i] = clock;
                match node.component {
                    Register { .. } if rising => storage[i] = values[node.inputs[0]],
                    Counter if rising => {
                        storage[i] = Signal::known(storage[i].value.wrapping_add(1), node.width)
                    }
                    Ram { address_bits } => {
                        let address = values[node.inputs[0]];
                        if address.known == bits(address_bits) {
                            if rising && values[node.inputs[2]] == Signal::known(1, 1) {
                                ram.insert((i, address.value), values[node.inputs[1]]);
                            }
                            storage[i] = *ram.get(&(i, address.value)).unwrap_or(&Signal::X);
                        } else {
                            storage[i] = Signal::X;
                        }
                    }
                    _ => {}
                }
                if storage[i] != values[i] {
                    pending.insert(plan.ranks[i]);
                }
            }
            self.settle(&plan, &mut pending, &mut values, &storage, tick, &mut wave)?;
            for (i, value) in values.iter().enumerate() {
                if tick == 0 || *value != previous[i] {
                    if wave.events.len() >= event_limit {
                        return Err("simulation event limit reached".into());
                    }
                    wave.events.push(Event {
                        tick,
                        node: i,
                        value: value.render(self.nodes[i].width),
                    });
                }
            }
        }
        Ok(wave)
    }
    fn settle(
        &self,
        plan: &Plan,
        pending: &mut BTreeSet<usize>,
        values: &mut [Signal],
        storage: &[Signal],
        tick: u32,
        wave: &mut Waveform,
    ) -> Result<()> {
        while let Some(rank) = pending.pop_first() {
            let i = plan.order[rank];
            let node = &self.nodes[i];
            let get = |j: usize| values[node.inputs[j]];
            let w = node.width;
            let m = bits(w);
            let known = |v| Signal::known(v, w);
            let invert = |s: Signal| Signal {
                value: !s.value & s.known,
                known: s.known,
                z: 0,
            };
            use Kind::*;
            let result = match &node.component {
                Input { value } => Signal::parse(value, w)?,
                Clock { period } => known(u32::from(tick % period >= period / 2)),
                Register { .. } | Counter | Ram { .. } => storage[i],
                Slice { lsb } => {
                    let input = get(0);
                    Signal {
                        value: (input.value >> lsb) & m,
                        known: (input.known >> lsb) & m,
                        z: (input.z >> lsb) & m,
                    }
                }
                Subcircuit { .. } => return Err("unexpanded subcircuit".into()),
                Output => get(0),
                Not => invert(get(0)),
                And | Nand | Or | Nor | Xor | Xnor => {
                    let a = get(0);
                    let b = get(1);
                    let a0 = a.known & !a.value;
                    let b0 = b.known & !b.value;
                    let (value, k) = match node.component {
                        And | Nand => (a.value & b.value, a0 | b0 | (a.known & b.known)),
                        Or | Nor => (a.value | b.value, a.value | b.value | (a.known & b.known)),
                        _ => (a.value ^ b.value, a.known & b.known),
                    };
                    let s = Signal {
                        value: value & k,
                        known: k,
                        z: 0,
                    };
                    if matches!(node.component, Nand | Nor | Xnor) {
                        invert(s)
                    } else {
                        s
                    }
                }
                Mux => match get(2) {
                    v if v == Signal::known(0, 1) => get(0),
                    v if v == Signal::known(1, 1) => get(1),
                    _ if get(0) == get(1) => get(0),
                    _ => Signal::X,
                },
                Demux => match get(1) {
                    v if v == Signal::known(0, 1) => known(0),
                    v if v == Signal::known(1, 1) => get(0),
                    _ => Signal::X,
                },
                Add | Carry | FullAdd | FullCarry | Equal => {
                    let a = get(0);
                    let b = get(1);
                    let width = self.nodes[node.inputs[0]].width;
                    let cin = if node.inputs.len() == 3 {
                        get(2)
                    } else {
                        Signal::known(0, 1)
                    };
                    if a.known != bits(width) || b.known != bits(width) || cin.known != 1 {
                        Signal::X
                    } else if matches!(node.component, Equal) {
                        known(u32::from(a.value == b.value))
                    } else {
                        let sum = u64::from(a.value) + u64::from(b.value) + u64::from(cin.value);
                        if matches!(node.component, Carry | FullCarry) {
                            known((sum >> width) as u32)
                        } else {
                            known(sum as u32)
                        }
                    }
                }
                Rom { words } => {
                    let address = get(0);
                    if address.known == bits(self.nodes[node.inputs[0]].width) {
                        words
                            .get(address.value as usize)
                            .map_or(Signal::X, |&v| known(v))
                    } else {
                        Signal::X
                    }
                }
                Resolve => {
                    let (mut ones, mut zeros, mut all_z, mut unknown) = (0, 0, m, 0);
                    for &j in &node.inputs {
                        let s = values[j];
                        ones |= s.value;
                        zeros |= s.known & !s.value;
                        all_z &= s.z;
                        unknown |= m & !(s.known | s.z);
                    }
                    let conflict = ones & zeros;
                    if conflict != 0
                        && !wave
                            .conflicts
                            .last()
                            .is_some_and(|(at, name)| *at == tick && name == &node.name)
                    {
                        if wave.conflicts.len() >= 100_000 {
                            return Err("simulation conflict limit reached".into());
                        }
                        wave.conflicts.push((tick, node.name.clone()));
                    }
                    let k = (ones | zeros) & !(conflict | unknown);
                    Signal {
                        value: ones & k,
                        known: k,
                        z: all_z,
                    }
                }
            };
            if values[i] != result {
                values[i] = result;
                pending.extend(plan.dependents[i].iter().map(|&j| plan.ranks[j]));
            }
        }
        Ok(())
    }
    pub fn vcd(&self, wave: &Waveform) -> String {
        use std::fmt::Write as _;
        let mut output = String::from(
            "$version SILICON v0.1.0-alpha $end\n$timescale 1ns $end\n$scope module silicon $end\n",
        );
        for (i, node) in self.nodes.iter().enumerate() {
            let _ = writeln!(output, "$var wire {} n{i} {} $end", node.width, node.name);
        }
        output.push_str("$upscope $end\n$enddefinitions $end\n");
        let mut tick = None;
        for event in &wave.events {
            if tick != Some(event.tick) {
                tick = Some(event.tick);
                let _ = writeln!(output, "#{}", event.tick);
            }
            if event.value.len() == 1 {
                let _ = writeln!(output, "{}n{}", event.value, event.node);
            } else {
                let _ = writeln!(output, "b{} n{}", event.value, event.node);
            }
        }
        let _ = writeln!(output, "#{}", wave.ticks);
        output
    }
}
