#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::{Deserialize, Serialize};
use silicon_core::{
    circuit::{Circuit, Waveform},
    cpu::{self, Machine},
    project::{self, Project, MAX_FILE},
    Result,
};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    sync::Mutex,
};
type Debugger = Mutex<Option<Machine>>;
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Step,
    Reverse,
    Run,
    Inspect,
}
#[derive(Serialize)]
struct Snapshot {
    state: cpu::State,
    trace: Vec<cpu::Trace>,
    memory: Vec<u8>,
    memory_base: usize,
    stack: Vec<u8>,
    console: String,
    stop: Option<cpu::Stop>,
    reversed: bool,
}
#[tauri::command]
fn template(name: &str) -> Result<Project> {
    project::reference(name)
}
#[tauri::command]
fn assemble(project: &str) -> Result<cpu::Assembly> {
    let p = Project::parse(project)?;
    cpu::assemble(&p.architecture, &p.source)
}
#[tauri::command]
fn prepare(state: tauri::State<'_, Debugger>, project: &str) -> Result<Vec<String>> {
    let p = Project::parse(project)?;
    let binary = cpu::assemble(&p.architecture, &p.source)?.binary;
    let disassembly = cpu::disassemble(&p.architecture, &binary)?;
    let machine = Machine::new(p.architecture, &binary, 4096)?;
    *state.lock().map_err(|_| "debugger lock unavailable")? = Some(machine);
    Ok(disassembly)
}
#[tauri::command]
fn debugger(
    state: tauri::State<'_, Debugger>,
    action: Action,
    budget: u64,
    breakpoints: Vec<u32>,
    watchpoints: Vec<u32>,
    address: usize,
) -> Result<Snapshot> {
    let mut guard = state.lock().map_err(|_| "debugger lock unavailable")?;
    let m = guard.as_mut().ok_or("prepare the program first")?;
    let mut stop = None;
    let mut reversed = false;
    match action {
        Action::Step => {
            m.step()?;
        }
        Action::Reverse => reversed = m.reverse(),
        Action::Run => {
            if budget > 2048 || breakpoints.len() > 4096 || watchpoints.len() > 4096 {
                return Err("debugger request exceeds limits".into());
            }
            stop = Some(m.run(
                budget,
                &breakpoints.into_iter().collect::<BTreeSet<_>>(),
                &watchpoints.into_iter().collect::<BTreeSet<_>>(),
            )?);
        }
        Action::Inspect => {}
    }
    let memory = m.memory();
    let base = address.min(memory.len() - 1);
    let stack = m
        .state()
        .sp
        .map(|sp| {
            let start = (sp as usize).min(memory.len());
            memory[start..(start + 32).min(memory.len())].to_vec()
        })
        .unwrap_or_default();
    Ok(Snapshot {
        state: m.state().clone(),
        trace: m.trace().rev().take(128).cloned().collect(),
        memory: memory[base..(base + 128).min(memory.len())].to_vec(),
        memory_base: base,
        stack,
        console: String::from_utf8_lossy(m.console()).into_owned(),
        stop,
        reversed,
    })
}
#[tauri::command]
fn simulate(project: &str, ticks: u32) -> Result<Waveform> {
    Project::parse(project)?.circuit.simulate(ticks, 100_000)
}
#[tauri::command]
async fn open_project() -> Result<Option<Project>> {
    let Some(path) = rfd::AsyncFileDialog::new()
        .add_filter("SILICON project", &["json"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let mut text = String::new();
    fs::File::open(path.path())
        .map_err(|e| e.to_string())?
        .take((MAX_FILE + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Project::parse(&text).map(Some)
}
fn save(path: &std::path::Path, data: &[u8]) -> Result<()> {
    let folder = path.parent().ok_or("missing destination directory")?;
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    file.write_all(data)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn save_project(project: String) -> Result<bool> {
    Project::parse(&project)?;
    let Some(path) = rfd::AsyncFileDialog::new()
        .add_filter("SILICON project", &["json"])
        .set_file_name("project.silicon.json")
        .save_file()
        .await
    else {
        return Ok(false);
    };
    save(path.path(), project.as_bytes())?;
    Ok(true)
}
#[tauri::command]
async fn export_vcd(project: String, ticks: u32) -> Result<bool> {
    let circuit: Circuit = Project::parse(&project)?.circuit;
    let wave = circuit.simulate(ticks, 100_000)?;
    let Some(path) = rfd::AsyncFileDialog::new()
        .add_filter("Value Change Dump", &["vcd"])
        .set_file_name("signals.vcd")
        .save_file()
        .await
    else {
        return Ok(false);
    };
    save(path.path(), circuit.vcd(&wave).as_bytes())?;
    Ok(true)
}
fn configured<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(Debugger::default())
        .invoke_handler(tauri::generate_handler![
            template,
            assemble,
            prepare,
            debugger,
            simulate,
            open_project,
            save_project,
            export_vcd
        ])
}

fn main() {
    configured(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("could not launch SILICON desktop");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test,
        webview::InvokeRequest,
    };

    fn request(
        window: &tauri::WebviewWindow<test::MockRuntime>,
        cmd: &str,
        body: Value,
    ) -> std::result::Result<Value, Value> {
        test::get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: test::INVOKE_KEY.to_string(),
            },
        )
        .map(|response| response.deserialize().unwrap())
    }
    #[test]
    fn project_assembly_simulation_and_debugger_use_registered_ipc() {
        let app = configured(test::mock_builder())
            .build(test::mock_context(test::noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let value = request(&window, "template", json!({"name":"micro-8"})).unwrap();
        let project = serde_json::to_string(&value).unwrap();
        let input = json!({"project":project});
        assert_eq!(
            request(&window, "assemble", input.clone()).unwrap()["symbols"]["loop"],
            8
        );
        let listing = request(&window, "prepare", input).unwrap();
        assert_eq!(listing[0], "SET r0, 0");
        let args = |action, budget| json!({"action":action,"budget":budget,"breakpoints":[],"watchpoints":[],"address":240});
        let snapshot = request(&window, "debugger", args("step", 0)).unwrap();
        assert_eq!(snapshot["state"]["pc"], 2);
        assert_eq!(
            request(&window, "debugger", args("reverse", 0)).unwrap()["state"]["pc"],
            0
        );
        assert_eq!(
            request(&window, "debugger", args("run", 256)).unwrap()["memory"][0],
            21
        );
        assert!(
            request(&window, "simulate", json!({"project":project,"ticks":8})).unwrap()["events"]
                .as_array()
                .unwrap()
                .len()
                > 6
        );
        assert!(request(&window, "simulate", json!({"project":project,"ticks":0})).is_err());
        assert!(request(&window, "assemble", json!({"project":"{}"})).is_err());
    }

    #[test]
    fn project_bytes_are_atomically_replaced_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");
        let first = project::reference("micro-8").unwrap();
        let second = project::reference("silicon-16").unwrap();
        for data in [first, second] {
            let json = serde_json::to_vec(&data).unwrap();
            save(&path, &json).unwrap();
            let reopened = Project::parse(&fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(reopened.name, data.name);
            reopened.validate().unwrap();
        }
    }
}
