use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Endian {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operand {
    Register,
    Unsigned,
    Signed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub lsb: u8,
    pub bits: u8,
    pub kind: Operand,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Halt,
    Nop,
    Mov,
    Imm,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Cmp,
    Load,
    Store,
    LoadIndirect,
    StoreIndirect,
    Jump,
    Jz,
    Jnz,
    Jcarry,
    Push,
    Pop,
    Call,
    Ret,
}
impl Op {
    fn operands(self) -> &'static [Operand] {
        use Operand::{Register as R, Unsigned as U};
        match self {
            Self::Halt | Self::Nop | Self::Ret => &[],
            Self::Push | Self::Pop => &[R],
            Self::Jump | Self::Jz | Self::Jnz | Self::Jcarry | Self::Call => &[U],
            Self::Imm | Self::Load | Self::Store => &[R, U],
            _ => &[R, R],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
    pub mnemonic: String,
    pub opcode: u32,
    pub mask: u32,
    pub fields: Vec<Field>,
    pub op: Op,
    #[serde(default)]
    pub update_flags: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Architecture {
    pub version: u8,
    pub name: String,
    pub data_bits: u8,
    pub address_bits: u8,
    pub registers: u8,
    pub instruction_bytes: u8,
    pub endian: Endian,
    pub entry: u32,
    pub stack: Option<u32>,
    pub console: Option<u32>,
    pub instructions: Vec<Instruction>,
}
fn mask(bits: u8) -> u32 {
    if bits == 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}
impl Architecture {
    pub fn memory_len(&self) -> usize {
        1usize << self.address_bits
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.name.trim().is_empty()
            || self.name.len() > 100
            || !matches!(self.data_bits, 8 | 16 | 32)
            || !(4..=20).contains(&self.address_bits)
            || !(1..=32).contains(&self.registers)
            || !(1..=4).contains(&self.instruction_bytes)
            || self.instructions.is_empty()
            || self.instructions.len() > 256
        {
            return Err("invalid architecture version or dimensions".into());
        }
        let len = self.memory_len() as u32;
        if self.entry >= len
            || self.stack.is_some_and(|s| s == 0 || s > len)
            || self.console.is_some_and(|s| s >= len)
        {
            return Err("invalid memory map".into());
        }
        let word_mask = mask(self.instruction_bytes * 8);
        let mut names = BTreeSet::new();
        for (i, ins) in self.instructions.iter().enumerate() {
            if !identifier(&ins.mnemonic)
                || !names.insert(ins.mnemonic.to_ascii_uppercase())
                || ins.mask == 0
                || ins.mask & !word_mask != 0
                || ins.opcode & !ins.mask != 0
                || ins.fields.len() != ins.op.operands().len()
            {
                return Err(format!("invalid encoding for {}", ins.mnemonic));
            }
            let mut used = ins.mask;
            for (field, expected) in ins.fields.iter().zip(ins.op.operands()) {
                if field.bits == 0
                    || u16::from(field.lsb) + u16::from(field.bits)
                        > u16::from(self.instruction_bytes) * 8
                    || (field.kind != *expected
                        && !(ins.op == Op::Imm && field.kind == Operand::Signed))
                    || (field.kind == Operand::Register
                        && mask(field.bits) < u32::from(self.registers) - 1)
                {
                    return Err(format!("invalid operand in {}", ins.mnemonic));
                }
                let bits = mask(field.bits) << field.lsb;
                if used & bits != 0 {
                    return Err(format!("overlapping fields in {}", ins.mnemonic));
                }
                used |= bits;
            }
            if used != word_mask {
                return Err(format!("unassigned bits in {}", ins.mnemonic));
            }
            if matches!(ins.op, Op::Push | Op::Pop | Op::Call | Op::Ret) && self.stack.is_none() {
                return Err(format!("{} requires a stack", ins.mnemonic));
            }
            for previous in &self.instructions[..i] {
                if (previous.opcode ^ ins.opcode) & previous.mask & ins.mask == 0 {
                    return Err(format!(
                        "overlapping encodings: {} and {}",
                        previous.mnemonic, ins.mnemonic
                    ));
                }
            }
        }
        Ok(())
    }
    fn encode_word(&self, word: u32, size: usize) -> Vec<u8> {
        match self.endian {
            Endian::Little => word.to_le_bytes()[..size].to_vec(),
            Endian::Big => word.to_be_bytes()[4 - size..].to_vec(),
        }
    }
    fn read_word(&self, bytes: &[u8]) -> u32 {
        let mut buf = [0; 4];
        match self.endian {
            Endian::Little => {
                buf[..bytes.len()].copy_from_slice(bytes);
                u32::from_le_bytes(buf)
            }
            Endian::Big => {
                buf[4 - bytes.len()..].copy_from_slice(bytes);
                u32::from_be_bytes(buf)
            }
        }
    }
    fn decode(&self, bytes: &[u8]) -> Result<(&Instruction, Vec<i64>)> {
        if bytes.len() != usize::from(self.instruction_bytes) {
            return Err("truncated instruction".into());
        }
        let word = self.read_word(bytes);
        let ins = self
            .instructions
            .iter()
            .find(|i| word & i.mask == i.opcode)
            .ok_or_else(|| format!("unknown instruction {word:#x}"))?;
        let mut values = Vec::with_capacity(ins.fields.len());
        for f in &ins.fields {
            let value = i64::from((word >> f.lsb) & mask(f.bits));
            if f.kind == Operand::Register && value >= i64::from(self.registers) {
                return Err("encoded register out of range".into());
            }
            let sign = 1i64 << (f.bits - 1);
            values.push(if f.kind == Operand::Signed && value & sign != 0 {
                value - (sign << 1)
            } else {
                value
            });
        }
        Ok((ins, values))
    }
    fn render(&self, bytes: &[u8]) -> Result<String> {
        let (ins, values) = self.decode(bytes)?;
        let arguments = ins
            .fields
            .iter()
            .zip(values)
            .map(|(field, value)| {
                if field.kind == Operand::Register {
                    format!("r{value}")
                } else {
                    value.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        Ok(if arguments.is_empty() {
            ins.mnemonic.clone()
        } else {
            format!("{} {arguments}", ins.mnemonic)
        })
    }
}
fn identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn number(s: &str) -> Option<i64> {
    let (negative, s) = s.strip_prefix('-').map_or((false, s), |s| (true, s));
    let value = if let Some(hex) = s.strip_prefix("0x") {
        i64::from_str_radix(hex, 16)
    } else if let Some(bin) = s.strip_prefix("0b") {
        i64::from_str_radix(bin, 2)
    } else {
        s.parse()
    }
    .ok()?;
    if negative {
        value.checked_neg()
    } else {
        Some(value)
    }
}
#[derive(Debug, Serialize)]
pub struct Assembly {
    pub binary: Vec<u8>,
    pub symbols: BTreeMap<String, i64>,
    pub source_map: BTreeMap<u32, usize>,
}

pub fn assemble(arch: &Architecture, source: &str) -> Result<Assembly> {
    arch.validate()?;
    if source.len() > 1_048_576 {
        return Err("source exceeds 1 MiB".into());
    }
    let mut symbols = BTreeMap::new();
    let mut lines = Vec::new();
    let mut address = arch.entry as usize;
    for (index, raw) in source.lines().enumerate() {
        let line = index + 1;
        let fail = |msg: &str| format!("{line}:1: {msg}");
        let mut input = raw.split([';', '#']).next().unwrap_or("").trim();
        if let Some((label, rest)) = input.split_once(':') {
            let label = label.trim();
            if !identifier(label) || symbols.insert(label.to_owned(), address as i64).is_some() {
                return Err(fail("invalid or duplicate label"));
            }
            input = rest.trim();
        }
        if input.is_empty() {
            continue;
        }
        let (head, tail) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        let args: Vec<_> = if tail.trim().is_empty() {
            Vec::new()
        } else {
            tail.split(',').map(str::trim).collect()
        };
        if head.eq_ignore_ascii_case(".equ") {
            if let [name, value] = args.as_slice() {
                if identifier(name) {
                    if let Some(value) = number(value) {
                        if symbols.insert((*name).to_owned(), value).is_none() {
                            continue;
                        }
                    }
                }
            }
            return Err(fail("expected .equ unique_name, numeric_literal"));
        }
        let size = match head.to_ascii_lowercase().as_str() {
            ".byte" => args.len(),
            ".word" => args.len() * usize::from(arch.data_bits / 8),
            _ => usize::from(arch.instruction_bytes),
        };
        if size == 0 || address + size > arch.memory_len() {
            return Err(fail("empty directive or program exceeds memory"));
        }
        lines.push((line, address, head, args));
        address += size;
    }
    let mut binary = Vec::with_capacity(address - arch.entry as usize);
    let mut source_map = BTreeMap::new();
    for (line, address, head, args) in lines {
        let fail = |msg: &str| format!("{line}:1: {msg}");
        let resolve = |arg: &str| {
            number(arg)
                .or_else(|| symbols.get(arg).copied())
                .ok_or_else(|| fail(&format!("unknown value {arg}")))
        };
        source_map.insert(address as u32, line);
        if matches!(head.to_ascii_lowercase().as_str(), ".byte" | ".word") {
            let bits = if head.eq_ignore_ascii_case(".byte") {
                8
            } else {
                arch.data_bits
            };
            for arg in args {
                let value = resolve(arg)?;
                if value < -(1i64 << (bits - 1)) || value > i64::from(mask(bits)) {
                    return Err(fail("data value out of range"));
                }
                binary.extend(arch.encode_word(value as u32, usize::from(bits / 8)));
            }
            continue;
        }
        let ins = arch
            .instructions
            .iter()
            .find(|i| i.mnemonic.eq_ignore_ascii_case(head))
            .ok_or_else(|| fail(&format!("unknown mnemonic {head}")))?;
        if args.len() != ins.fields.len() {
            return Err(fail("wrong operand count"));
        }
        let mut word = ins.opcode;
        for (f, arg) in ins.fields.iter().zip(args) {
            let value = if f.kind == Operand::Register {
                arg.strip_prefix(['r', 'R'])
                    .and_then(|r| r.parse::<u8>().ok())
                    .filter(|r| *r < arch.registers)
                    .map(i64::from)
                    .ok_or_else(|| fail("invalid register"))?
            } else {
                resolve(arg)?
            };
            let (min, max) = if f.kind == Operand::Signed {
                let sign = 1i64 << (f.bits - 1);
                (-sign, sign - 1)
            } else {
                (0, i64::from(mask(f.bits)))
            };
            if !(min..=max).contains(&value) {
                return Err(fail("operand out of range"));
            }
            word |= ((value as u32) & mask(f.bits)) << f.lsb;
        }
        binary.extend(arch.encode_word(word, usize::from(arch.instruction_bytes)));
    }
    Ok(Assembly {
        binary,
        symbols,
        source_map,
    })
}
pub fn disassemble(arch: &Architecture, binary: &[u8]) -> Result<Vec<String>> {
    arch.validate()?;
    if binary.len() > arch.memory_len() {
        return Err("binary exceeds memory".into());
    }
    Ok(binary
        .chunks(usize::from(arch.instruction_bytes))
        .map(|bytes| {
            arch.render(bytes).unwrap_or_else(|_| {
                format!(
                    ".byte {}",
                    bytes
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        })
        .collect())
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Flags {
    pub zero: bool,
    pub negative: bool,
    pub carry: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct State {
    pub pc: u32,
    pub sp: Option<u32>,
    pub registers: Vec<u32>,
    pub flags: Flags,
    pub cycles: u64,
    pub halted: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Trace {
    pub cycle: u64,
    pub pc: u32,
    pub instruction: String,
    pub reads: Vec<u32>,
    pub writes: Vec<u32>,
}
struct Delta {
    pc: u32,
    sp: Option<u32>,
    flags: Flags,
    register: Option<(usize, u32)>,
    memory: Vec<(usize, u8)>,
    console_len: usize,
    halted: bool,
    trace: Trace,
}
pub struct Machine {
    arch: Architecture,
    state: State,
    memory: Vec<u8>,
    console: Vec<u8>,
    history: VecDeque<Delta>,
    capacity: usize,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    Halted,
    Budget,
    Breakpoint,
    Watchpoint,
}
impl Machine {
    pub fn new(arch: Architecture, binary: &[u8], history: usize) -> Result<Self> {
        arch.validate()?;
        let mut memory = vec![0; arch.memory_len()];
        let entry = arch.entry as usize;
        if binary.len() > memory.len() - entry || history > 100_000 {
            return Err("program or history exceeds limits".into());
        }
        memory[entry..entry + binary.len()].copy_from_slice(binary);
        let state = State {
            pc: arch.entry,
            sp: arch.stack,
            registers: vec![0; usize::from(arch.registers)],
            flags: Flags::default(),
            cycles: 0,
            halted: false,
        };
        Ok(Self {
            arch,
            state,
            memory,
            console: Vec::new(),
            history: VecDeque::new(),
            capacity: history,
        })
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn memory(&self) -> &[u8] {
        &self.memory
    }
    pub fn console(&self) -> &[u8] {
        &self.console
    }
    pub fn trace(&self) -> impl DoubleEndedIterator<Item = &Trace> {
        self.history.iter().map(|d| &d.trace)
    }
    pub fn step(&mut self) -> Result<Option<Trace>> {
        if self.state.halted {
            return Ok(None);
        }
        let pc = self.state.pc as usize;
        let next = pc + usize::from(self.arch.instruction_bytes);
        let bytes = self
            .memory
            .get(pc..next)
            .ok_or("instruction fetch outside memory")?;
        let (ins, args) = self.arch.decode(bytes)?;
        let mut delta = Delta {
            pc: self.state.pc,
            sp: self.state.sp,
            flags: self.state.flags,
            register: None,
            memory: Vec::new(),
            console_len: self.console.len(),
            halted: self.state.halted,
            trace: Trace {
                cycle: self.state.cycles,
                pc: self.state.pc,
                instruction: self.arch.render(bytes)?,
                reads: Vec::new(),
                writes: Vec::new(),
            },
        };
        let mut next_pc = next as u32;
        let mut sp = self.state.sp;
        let (mut register, mut write, mut flags, mut halted) = (None, None, None, false);
        let data_mask = mask(self.arch.data_bits);
        let size = usize::from(self.arch.data_bits / 8);
        let arg = |i| args.get(i).copied().unwrap_or(0) as u32;
        let reg = |i| self.state.registers[arg(i) as usize];
        let mut target = |address: u32| -> Result<()> {
            if address as usize >= self.memory.len() {
                return Err("branch outside memory".into());
            }
            next_pc = address;
            Ok(())
        };
        let mut read = |address: u32, size: usize| -> Result<u32> {
            let start = address as usize;
            let data = self
                .memory
                .get(start..start + size)
                .ok_or("read outside memory")?;
            delta.trace.reads.extend(address..address + size as u32);
            Ok(self.arch.read_word(data))
        };
        use Op::*;
        let mut carry = false;
        let value = match ins.op {
            Mov => Some(reg(1)),
            Imm => Some(arg(1)),
            Add => {
                let sum = u64::from(reg(0)) + u64::from(reg(1));
                carry = sum > u64::from(data_mask);
                Some(sum as u32)
            }
            Sub | Cmp => {
                carry = reg(0) >= reg(1);
                Some(reg(0).wrapping_sub(reg(1)))
            }
            And => Some(reg(0) & reg(1)),
            Or => Some(reg(0) | reg(1)),
            Xor => Some(reg(0) ^ reg(1)),
            Shl => Some(reg(0).checked_shl(reg(1)).unwrap_or(0)),
            Shr => Some(reg(0).checked_shr(reg(1)).unwrap_or(0)),
            Load => Some(read(arg(1), size)?),
            LoadIndirect => Some(read(reg(1), size)?),
            Store | StoreIndirect => {
                write = Some((if ins.op == Store { arg(1) } else { reg(1) }, reg(0), size));
                None
            }
            Jump | Jz | Jnz | Jcarry => {
                if ins.op == Jump
                    || (ins.op == Jz && self.state.flags.zero)
                    || (ins.op == Jnz && !self.state.flags.zero)
                    || (ins.op == Jcarry && self.state.flags.carry)
                {
                    target(arg(0))?;
                }
                None
            }
            Push | Call => {
                let width = if ins.op == Call { 4 } else { size };
                let address = sp
                    .and_then(|s| s.checked_sub(width as u32))
                    .ok_or("stack overflow")?;
                write = Some((
                    address,
                    if ins.op == Call { next as u32 } else { reg(0) },
                    width,
                ));
                sp = Some(address);
                if ins.op == Call {
                    target(arg(0))?;
                }
                None
            }
            Pop | Ret => {
                let address = sp.ok_or("stack unavailable")?;
                let width = if ins.op == Ret { 4 } else { size };
                if address + width as u32 > self.arch.stack.ok_or("stack unavailable")? {
                    return Err("stack underflow".into());
                }
                let value = read(address, width)?;
                sp = Some(address + width as u32);
                if ins.op == Ret {
                    target(value)?;
                    None
                } else {
                    Some(value)
                }
            }
            Halt => {
                halted = true;
                None
            }
            Nop => None,
        };
        if let Some(value) = value {
            let result = value & data_mask;
            if ins.update_flags {
                flags = Some(Flags {
                    zero: result == 0,
                    negative: result & (1 << (self.arch.data_bits - 1)) != 0,
                    carry,
                });
            }
            if ins.op != Cmp {
                register = Some((arg(0) as usize, result));
            }
        }
        let mut console_byte = None;
        if let Some((address, value, width)) = write {
            let start = address as usize;
            if start + width > self.memory.len() {
                return Err("write outside memory".into());
            }
            delta.trace.writes.extend(address..address + width as u32);
            if self.arch.console == Some(address) && matches!(ins.op, Store | StoreIndirect) {
                if self.console.len() >= 1_048_576 {
                    return Err("console limit reached".into());
                }
                console_byte = Some(value as u8);
            } else {
                for (offset, byte) in self.arch.encode_word(value, width).into_iter().enumerate() {
                    let index = start + offset;
                    delta.memory.push((index, self.memory[index]));
                    self.memory[index] = byte;
                }
            }
        }
        if let Some(byte) = console_byte {
            self.console.push(byte);
        }
        if let Some((index, value)) = register {
            delta.register = Some((index, self.state.registers[index]));
            self.state.registers[index] = value;
        }
        if let Some(flags) = flags {
            self.state.flags = flags;
        }
        self.state.pc = next_pc;
        self.state.sp = sp;
        self.state.halted = halted;
        self.state.cycles += 1;
        let trace = delta.trace.clone();
        if self.capacity > 0 {
            if self.history.len() == self.capacity {
                self.history.pop_front();
            }
            self.history.push_back(delta);
        }
        Ok(Some(trace))
    }
    pub fn reverse(&mut self) -> bool {
        let Some(delta) = self.history.pop_back() else {
            return false;
        };
        self.state.pc = delta.pc;
        self.state.sp = delta.sp;
        self.state.flags = delta.flags;
        self.state.halted = delta.halted;
        self.state.cycles = delta.trace.cycle;
        if let Some((index, value)) = delta.register {
            self.state.registers[index] = value;
        }
        for (index, value) in delta.memory {
            self.memory[index] = value;
        }
        self.console.truncate(delta.console_len);
        true
    }
    pub fn run(
        &mut self,
        budget: u64,
        breakpoints: &BTreeSet<u32>,
        watchpoints: &BTreeSet<u32>,
    ) -> Result<Stop> {
        if budget > 10_000_000 {
            return Err("execution budget exceeds 10 million instructions".into());
        }
        for _ in 0..budget {
            if self.state.halted {
                return Ok(Stop::Halted);
            }
            if breakpoints.contains(&self.state.pc) {
                return Ok(Stop::Breakpoint);
            }
            if let Some(trace) = self.step()? {
                if trace
                    .reads
                    .iter()
                    .chain(&trace.writes)
                    .any(|a| watchpoints.contains(a))
                {
                    return Ok(Stop::Watchpoint);
                }
            }
        }
        Ok(if self.state.halted {
            Stop::Halted
        } else {
            Stop::Budget
        })
    }
}
