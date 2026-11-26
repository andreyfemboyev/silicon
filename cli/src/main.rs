use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use silicon_core::{
    cpu::{self, Architecture, Machine},
    project::{self, Project, MAX_FILE},
    Result,
};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
#[command(version, about = "SILICON — Architecture Development Studio")]
struct Args {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum ArchCommand {
    Inspect { file: PathBuf },
}
#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(default_value = "project.silicon.json")]
        file: PathBuf,
        #[arg(long, default_value = "micro-8")]
        reference: String,
    },
    Validate {
        file: PathBuf,
    },
    Arch {
        #[command(subcommand)]
        command: ArchCommand,
    },
    Assemble {
        file: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        symbols: Option<PathBuf>,
    },
    Disassemble {
        file: PathBuf,
        binary: PathBuf,
    },
    Run {
        file: PathBuf,
        #[arg(long, default_value_t = 10_000)]
        budget: u64,
        #[arg(long)]
        breakpoint: Vec<u32>,
        #[arg(long)]
        watchpoint: Vec<u32>,
        #[arg(long)]
        memory: Option<u32>,
    },
    Trace {
        file: PathBuf,
        #[arg(long, default_value_t = 10_000)]
        budget: u64,
        #[arg(long, default_value_t = 256)]
        history: usize,
    },
    Simulate {
        file: PathBuf,
        #[arg(long, default_value_t = 16)]
        ticks: u32,
        #[arg(long)]
        vcd: Option<PathBuf>,
    },
    Test,
    Benchmark {
        #[arg(long, default_value_t = 100)]
        iterations: u32,
    },
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("{} exceeds size limit", path.display()));
    }
    Ok(bytes)
}
fn input(path: &Path) -> Result<String> {
    String::from_utf8(read(path, MAX_FILE)?).map_err(|e| e.to_string())
}
fn project(path: &Path) -> Result<Project> {
    Project::parse(&input(path)?)
}
fn arch_and_source(path: &Path) -> Result<(Architecture, String)> {
    let data = input(path)?;
    if let Ok(p) = Project::parse(&data) {
        return Ok((p.architecture, p.source));
    }
    let arch: Architecture = serde_json::from_str(&data)
        .map_err(|e| format!("expected architecture or project: {e}"))?;
    arch.validate()?;
    Ok((arch, String::new()))
}
fn write(path: &Path, data: impl AsRef<[u8]>) -> Result<()> {
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}
fn pretty(value: &impl serde::Serialize) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| e.to_string())
}
fn report_state(machine: &Machine) -> String {
    let s = machine.state();
    let regs = s
        .registers
        .iter()
        .enumerate()
        .map(|(i, r)| format!("r{i}={r:#x}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "PC={:#x} SP={:?} cycles={} halted={}\n{regs}\nZ={} N={} C={}\n{}",
        s.pc,
        s.sp,
        s.cycles,
        s.halted,
        s.flags.zero,
        s.flags.negative,
        s.flags.carry,
        String::from_utf8_lossy(machine.console())
    )
}
fn execute(command: Command) -> Result<(String, Value)> {
    match command {
        Command::Init { file, reference } => {
            use std::io::Write;
            let data = pretty(&project::reference(&reference)?)?;
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&file)
                .and_then(|mut f| f.write_all(data.as_bytes()))
                .map_err(|e| format!("{}: {e}", file.display()))?;
            Ok((
                format!("Created {}", file.display()),
                json!({"file":file,"reference":reference}),
            ))
        }
        Command::Validate { file } => {
            let data = input(&file)?;
            let size = match Project::parse(&data) {
                Ok(p) => Some(p.validate()?.binary.len()),
                Err(project_error) => {
                    let arch: Architecture =
                        serde_json::from_str(&data).map_err(|_| project_error)?;
                    arch.validate()?;
                    None
                }
            };
            Ok((
                format!("Valid {}", file.display()),
                json!({"valid":true,"binary_bytes":size}),
            ))
        }
        Command::Arch {
            command: ArchCommand::Inspect { file },
        } => {
            let (a, _) = arch_and_source(&file)?;
            a.validate()?;
            let instructions = a
                .instructions
                .iter()
                .map(|i| i.mnemonic.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Ok((
                format!(
                    "{} · {}-bit data · {}-bit addresses · {} registers\n{instructions}",
                    a.name, a.data_bits, a.address_bits, a.registers
                ),
                json!(a),
            ))
        }
        Command::Assemble {
            file,
            source,
            output,
            symbols,
        } => {
            let (a, embedded) = arch_and_source(&file)?;
            let source = source
                .as_deref()
                .map(input)
                .transpose()?
                .unwrap_or(embedded);
            let assembled = cpu::assemble(&a, &source)?;
            if symbols.as_ref() == Some(&output) {
                return Err("symbol map and binary paths must differ".into());
            }
            write(&output, &assembled.binary)?;
            if let Some(path) = symbols {
                write(&path, pretty(&assembled.symbols)?)?;
            }
            Ok((
                format!(
                    "Wrote {} bytes to {}",
                    assembled.binary.len(),
                    output.display()
                ),
                json!(assembled),
            ))
        }
        Command::Disassemble { file, binary } => {
            let (a, _) = arch_and_source(&file)?;
            a.validate()?;
            let lines = cpu::disassemble(&a, &read(&binary, a.memory_len())?)?;
            Ok((lines.join("\n"), json!(lines)))
        }
        Command::Run {
            file,
            budget,
            breakpoint,
            watchpoint,
            memory,
        } => {
            let p = project(&file)?;
            let binary = p.validate()?.binary;
            let mut m = Machine::new(p.architecture, &binary, 0)?;
            let memory = memory
                .map(|address| -> Result<usize> {
                    let start = address as usize;
                    if start >= m.memory().len() {
                        return Err("memory view starts outside memory".into());
                    }
                    Ok(start)
                })
                .transpose()?;
            let stop = m.run(
                budget,
                &breakpoint.into_iter().collect(),
                &watchpoint.into_iter().collect(),
            )?;
            let bytes = memory.map(|start| &m.memory()[start..(start + 128).min(m.memory().len())]);
            let mut display = format!("{stop:?}\n{}", report_state(&m));
            if let Some(bytes) = bytes {
                display.push_str(&format!("\n{:02x?}", bytes));
            }
            Ok((
                display,
                json!({"stop":stop,"state":m.state(),"console":m.console(),"memory":bytes}),
            ))
        }
        Command::Trace {
            file,
            budget,
            history,
        } => {
            let p = project(&file)?;
            let binary = p.validate()?.binary;
            let mut m = Machine::new(p.architecture, &binary, history)?;
            let stop = m.run(budget, &BTreeSet::new(), &BTreeSet::new())?;
            let trace = m.trace().collect::<Vec<_>>();
            let display = trace
                .iter()
                .map(|t| format!("{:>6}  {:08x}  {}", t.cycle, t.pc, t.instruction))
                .collect::<Vec<_>>()
                .join("\n");
            Ok((
                format!("{display}\n{stop:?}"),
                json!({"stop":stop,"trace":trace,"state":m.state()}),
            ))
        }
        Command::Simulate { file, ticks, vcd } => {
            let p = project(&file)?;
            let wave = p.circuit.simulate(ticks, 100_000)?;
            if let Some(path) = vcd {
                write(&path, p.circuit.vcd(&wave))?;
            }
            let display = wave
                .events
                .iter()
                .take(200)
                .map(|e| {
                    format!(
                        "{:>6}  {:<20} {}",
                        e.tick, p.circuit.nodes[e.node].name, e.value
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok((
                format!(
                    "{} events · {} conflicts\n{display}",
                    wave.events.len(),
                    wave.conflicts.len()
                ),
                json!(wave),
            ))
        }
        Command::Test => {
            let mut result = Vec::new();
            for name in ["micro-8", "silicon-16", "silicon-32"] {
                let p = project::reference(name)?;
                let binary = p.validate()?.binary;
                let mut m = Machine::new(p.architecture, &binary, 1024)?;
                if m.run(1000, &BTreeSet::new(), &BTreeSet::new())? != cpu::Stop::Halted {
                    return Err(format!("{name} did not halt"));
                }
                let pass = match name {
                    "micro-8" => m.memory()[240] == 21,
                    "silicon-16" => m.console() == b"A\n",
                    _ => m.memory()[256..268] == [3, 0, 0, 0, 5, 0, 0, 0, 8, 0, 0, 0],
                };
                if !pass {
                    return Err(format!("{name} computed an unexpected result"));
                }
                let events = p.circuit.simulate(16, 1000)?.events.len();
                result.push(json!({"reference":name,"cycles":m.state().cycles,"events":events}));
            }
            Ok((
                String::from("Three reference programs and circuits passed"),
                json!(result),
            ))
        }
        Command::Benchmark { iterations } => {
            if !(1..=10_000).contains(&iterations) {
                return Err("iterations must be in 1..=10000".into());
            }
            let p = project::reference("micro-8")?;
            let binary = p.validate()?.binary;
            let start = Instant::now();
            let mut steps = 0;
            for _ in 0..iterations {
                let mut m = Machine::new(p.architecture.clone(), &binary, 0)?;
                m.run(1000, &BTreeSet::new(), &BTreeSet::new())?;
                steps += m.state().cycles;
                std::hint::black_box(m.state());
            }
            let cpu_ns = start.elapsed().as_nanos() as u64;
            let start = Instant::now();
            let mut events = 0;
            for _ in 0..iterations {
                events += std::hint::black_box(p.circuit.simulate(64, 1000)?)
                    .events
                    .len();
            }
            let circuit_ns = start.elapsed().as_nanos() as u64;
            Ok((
                format!("{steps} instructions in {cpu_ns} ns\n{events} events in {circuit_ns} ns"),
                json!({"iterations":iterations,"instructions":steps,"cpu_ns":cpu_ns,"events":events,"circuit_ns":circuit_ns}),
            ))
        }
    }
}
fn main() -> std::process::ExitCode {
    let args = Args::parse();
    match execute(args.command) {
        Ok((message, value)) => {
            if args.json {
                println!("{value}");
            } else {
                println!("{message}");
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            if args.json {
                eprintln!("{}", json!({"error":error}));
            } else {
                eprintln!("error: {error}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}
