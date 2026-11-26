use silicon_core::{
    circuit::{Circuit, Kind, Node, Waveform},
    cpu::{self, Machine, Stop},
    project::{self, Project},
};
use std::collections::BTreeSet;

fn node(name: &str, width: u8, component: Kind, inputs: &[usize]) -> Node {
    Node {
        name: name.into(),
        width,
        component,
        inputs: inputs.to_vec(),
        x: 0,
        y: 0,
    }
}
fn literal(name: &str, bits: &str) -> Node {
    node(
        name,
        bits.len() as u8,
        Kind::Input { value: bits.into() },
        &[],
    )
}
fn circuit(nodes: Vec<Node>) -> Circuit {
    Circuit { version: 1, nodes }
}
fn sample(w: &Waveform, node: usize, tick: u32) -> &str {
    &w.events
        .iter()
        .rev()
        .find(|e| e.node == node && e.tick <= tick)
        .unwrap()
        .value
}
#[test]
fn exhaustive_four_state_gates() {
    let values = ["0", "1", "x", "z"];
    let tables = [
        (Kind::And, ["0000", "01xx", "0xxx", "0xxx"]),
        (Kind::Or, ["01xx", "1111", "x1xx", "x1xx"]),
        (Kind::Xor, ["01xx", "10xx", "xxxx", "xxxx"]),
        (Kind::Nand, ["1111", "10xx", "1xxx", "1xxx"]),
        (Kind::Nor, ["10xx", "0000", "x0xx", "x0xx"]),
        (Kind::Xnor, ["10xx", "01xx", "xxxx", "xxxx"]),
    ];
    for (gate, truth) in tables {
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                let built = circuit(vec![
                    literal("a", a),
                    literal("b", b),
                    node("gate", 1, gate.clone(), &[0, 1]),
                ]);
                let wave = built.simulate(1, 10).unwrap();
                assert_eq!(
                    sample(&wave, 2, 0).as_bytes()[0],
                    truth[i].as_bytes()[j],
                    "{gate:?} {a} {b}"
                );
            }
        }
    }
    for (input, expected) in values.into_iter().zip(["1", "0", "x", "x"]) {
        let wave = circuit(vec![
            literal("input", input),
            node("not", 1, Kind::Not, &[0]),
        ])
        .simulate(1, 10)
        .unwrap();
        assert_eq!(sample(&wave, 1, 0), expected);
    }
}
#[test]
fn registers_sample_simultaneously_and_ram_records_writes() {
    let built = circuit(vec![
        node("clk", 1, Kind::Clock { period: 2 }, &[]),
        literal("addr", "0"),
        node("a", 8, Kind::Register { initial: Some(7) }, &[3, 0]),
        node("b", 8, Kind::Register { initial: Some(9) }, &[2, 0]),
        literal("we", "1"),
        node("ram", 8, Kind::Ram { address_bits: 1 }, &[1, 2, 4, 0]),
    ]);
    let w = built.simulate(5, 100).unwrap();
    assert_eq!(sample(&w, 2, 1), "00001001");
    assert_eq!(sample(&w, 3, 1), "00000111");
    assert_eq!(sample(&w, 5, 0), "xxxxxxxx");
    assert_eq!(sample(&w, 5, 1), "00000111");
    assert_eq!(sample(&w, 5, 3), "00001001");
    assert_eq!(w.events, built.simulate(5, 100).unwrap().events);
    assert!(built.vcd(&w).contains("#3\n"));
}
#[test]
fn resolver_detects_contention_and_preserves_high_impedance() {
    let built = circuit(vec![
        literal("a", "0zz1"),
        literal("b", "1zz1"),
        node("bus", 4, Kind::Resolve, &[0, 1]),
    ]);
    let w = built.simulate(2, 20).unwrap();
    assert_eq!(w.conflicts, vec![(0, "bus".to_string())]);
    assert_eq!(sample(&w, 2, 1), "xzz1");
}
#[test]
fn nested_subcircuits_connect_by_port_and_detect_cross_boundary_loops() {
    let inverter = circuit(vec![literal("input", "0"), node("not", 1, Kind::Not, &[0])]);
    let make = |c: Circuit| Kind::Subcircuit {
        circuit: Box::new(c),
        ports: vec![0],
        output: 1,
    };
    let double = circuit(vec![
        literal("port", "0"),
        node("nested", 1, make(inverter), &[0]),
    ]);
    let mut root = circuit(vec![
        literal("constant", "1"),
        node("module", 1, make(double), &[0]),
    ]);
    assert_eq!(sample(&root.simulate(1, 10).unwrap(), 1, 0), "0");
    root.nodes[1].inputs = vec![1];
    assert!(root.validate().unwrap_err().contains("loop"));
}
#[test]
fn circuit_rejects_loops_bad_width_and_limits() {
    assert!(circuit(vec![node("a", 1, Kind::Not, &[0])])
        .validate()
        .unwrap_err()
        .contains("loop"));
    assert!(
        circuit(vec![literal("a", "10"), node("b", 1, Kind::Not, &[0])])
            .validate()
            .is_err()
    );
    assert!(circuit(vec![literal("z", "zz")]).simulate(1, 0).is_err());
    assert!(circuit(vec![literal("z", "zz")])
        .simulate(100_001, 10)
        .is_err());
    assert!(circuit(vec![node("a", 1, Kind::Not, &[usize::MAX])])
        .validate()
        .is_err());
}
#[test]
fn reference_programs_round_trip_and_execute() {
    for name in ["micro-8", "silicon-16", "silicon-32"] {
        let p = project::reference(name).unwrap();
        let a = p.validate().unwrap();
        let lines = cpu::disassemble(&p.architecture, &a.binary)
            .unwrap()
            .join("\n");
        assert_eq!(
            cpu::assemble(&p.architecture, &lines).unwrap().binary,
            a.binary
        );
        let encoded = serde_json::to_string(&p).unwrap();
        assert_eq!(
            Project::parse(&encoded).unwrap().validate().unwrap().binary,
            a.binary
        );
        let mut m = Machine::new(p.architecture, &a.binary, 128).unwrap();
        assert_eq!(
            m.run(1000, &BTreeSet::new(), &BTreeSet::new()).unwrap(),
            Stop::Halted
        );
        match name {
            "micro-8" => assert_eq!(m.memory()[240], 21),
            "silicon-16" => assert_eq!(m.console(), b"A\n"),
            _ => assert_eq!(&m.memory()[256..268], &[3, 0, 0, 0, 5, 0, 0, 0, 8, 0, 0, 0]),
        }
    }
}
#[test]
fn micro8_circuit_matches_cpu_after_each_retired_instruction() {
    let p = project::reference("micro-8-cpu").unwrap();
    let binary = p.validate().unwrap().binary;
    let mut m = Machine::new(p.architecture, &binary, 10).unwrap();
    let w = p.circuit.simulate(19, 1600).unwrap();
    let names = [
        "pc", "r0", "r1", "r2", "r3", "halted", "zero", "negative", "carry",
    ];
    let observed = |name: &str, tick| {
        let node = p.circuit.nodes.iter().position(|n| n.name == name).unwrap();
        u32::from_str_radix(sample(&w, node, tick), 2).unwrap()
    };
    for tick in 0..19 {
        if tick % 2 == 1 {
            m.step().unwrap();
        }
        let s = m.state();
        let expected = [
            s.pc,
            s.registers[0],
            s.registers[1],
            s.registers[2],
            s.registers[3],
            u32::from(s.halted),
            u32::from(s.flags.zero),
            u32::from(s.flags.negative),
            u32::from(s.flags.carry),
        ];
        for (name, value) in names.into_iter().zip(expected) {
            assert_eq!(observed(name, tick), value, "{name} at tick {tick}");
        }
    }
}
#[test]
fn exhaustive_8_bit_add_sub_and_flags() {
    let arch = project::reference("micro-8").unwrap().architecture;
    for (op, formula) in [
        ("ADD", u8::overflowing_add as fn(u8, u8) -> (u8, bool)),
        ("SUB", u8::overflowing_sub),
    ] {
        let prefix = cpu::assemble(&arch, &format!("SET r0, 0\nSET r1, 0\n{op} r0, r1"))
            .unwrap()
            .binary;
        let mut program = prefix;
        for a in 0..=u8::MAX {
            for b in 0..=u8::MAX {
                program[0] = a;
                program[2] = b;
                let mut m = Machine::new(arch.clone(), &program, 0).unwrap();
                for _ in 0..3 {
                    m.step().unwrap();
                }
                let (value, overflow) = formula(a, b);
                assert_eq!(m.state().registers[0], u32::from(value));
                let flags = m.state().flags;
                assert_eq!(
                    (flags.zero, flags.negative, flags.carry),
                    (value == 0, value >= 128, overflow ^ (op == "SUB"))
                );
            }
        }
    }
}
#[test]
fn debug_break_watch_reverse_and_replay() {
    let p = project::reference("silicon-16").unwrap();
    let a = p.validate().unwrap();
    let mut m = Machine::new(p.architecture.clone(), &a.binary, 128).unwrap();
    assert_eq!(
        m.run(10, &BTreeSet::from([0]), &BTreeSet::new()).unwrap(),
        Stop::Breakpoint
    );
    assert_eq!(m.state().cycles, 0);
    assert_eq!(
        m.run(
            99,
            &BTreeSet::new(),
            &BTreeSet::from([p.architecture.console.unwrap()])
        )
        .unwrap(),
        Stop::Watchpoint
    );
    assert_eq!(m.console(), b"A");
    let watched = m.state().clone();
    let memory = m.memory().to_vec();
    let trace = m.trace().last().unwrap().clone();
    assert!(m.reverse());
    assert!(m.console().is_empty());
    assert_eq!(m.step().unwrap().unwrap(), trace);
    assert_eq!(*m.state(), watched);
    assert_eq!(m.memory(), memory);
    while m.reverse() {}
    let fresh = Machine::new(p.architecture, &a.binary, 0).unwrap();
    assert_eq!(m.state(), fresh.state());
    assert_eq!(m.memory(), fresh.memory());
    assert_eq!(m.console(), fresh.console());
}
#[test]
fn invalid_encoding_assembly_and_fault_are_bounded() {
    let mut a = project::reference("micro-8").unwrap().architecture;
    let binary = cpu::assemble(&a, "LD r0, 257").unwrap().binary;
    let mut m = Machine::new(a.clone(), &binary, 1).unwrap();
    let before = m.state().clone();
    assert!(m.step().is_err());
    assert_eq!(*m.state(), before);
    assert!(!m.reverse());
    assert!(cpu::assemble(&a, "SET r99, 1")
        .unwrap_err()
        .starts_with("1:1:"));
    assert!(cpu::assemble(&a, "JMP missing").is_err());
    let mut dup = a.instructions[0].clone();
    dup.mnemonic = "OTHER".into();
    a.instructions.push(dup);
    assert!(a.validate().unwrap_err().contains("overlapping"));
    assert!(Project::parse("{\"version\":2}").is_err());
}
#[test]
fn big_endian_signed_fields_and_forward_constant() {
    let mut a = project::reference("micro-8").unwrap().architecture;
    a.endian = cpu::Endian::Big;
    a.instructions
        .iter_mut()
        .find(|i| i.mnemonic == "SET")
        .unwrap()
        .fields[1]
        .kind = cpu::Operand::Signed;
    let b = cpu::assemble(&a, "SET r3, SIGNED\nHALT\n.equ SIGNED, -0x2")
        .unwrap()
        .binary;
    assert_eq!(b, [0x1f, 0xfe, 0, 0]);
    assert_eq!(cpu::disassemble(&a, &b).unwrap(), ["SET r3, -2", "HALT"]);
    let mut m = Machine::new(a, &b, 0).unwrap();
    m.step().unwrap();
    assert_eq!(m.state().registers[3], 254);
}
