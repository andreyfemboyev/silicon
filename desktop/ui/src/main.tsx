import { useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import Editor, { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import Worker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { Schematic } from "./Schematic";
import { Signals } from "./Signals";
import { hex } from "./model";
import type { Assembly, Circuit, Project, Snapshot, Wave } from "./model";
import "./style.css";

Object.assign(globalThis, {
  MonacoEnvironment: { getWorker: () => new Worker() },
});
loader.config({ monaco });
monaco.languages.register({ id: "silicon-asm" });
monaco.languages.setMonarchTokensProvider("silicon-asm", {
  tokenizer: {
    root: [
      [/[;#].*$/, "comment"],
      [/[a-z_]\w*:/i, "type.identifier"],
      [/\br\d+\b/i, "variable"],
      [/\.[a-z]+/i, "keyword"],
      [/(-?0x[0-9a-f]+|-?0b[01]+|-?\d+)\b/i, "number"],
      [/^\s*[a-z]+/i, "keyword"],
    ],
  },
});
monaco.editor.defineTheme("silicon", {
  base: "vs-dark",
  inherit: true,
  rules: [
    { token: "keyword", foreground: "89F0BD" },
    { token: "variable", foreground: "75CEEA" },
    { token: "comment", foreground: "83969D" },
    { token: "number", foreground: "EDC88C" },
  ],
  colors: {
    "editor.background": "#101518",
    "editorGutter.background": "#101518",
    "editorLineNumber.foreground": "#789098",
    "editorCursor.foreground": "#90FFD0",
    "editor.lineHighlightBackground": "#1C282D",
  },
});

const references = ["micro-8", "micro-8-cpu", "silicon-16", "silicon-32"];
type View = "circuit" | "source" | "architecture" | "debugger" | "signals";
type DebugAction = "step" | "reverse" | "run" | "inspect";
const numericList = (s: string) =>
  s.trim()
    ? s.split(",").map((x) => {
        const value = Number(x.trim());
        if (
          !x.trim() ||
          !Number.isInteger(value) ||
          value < 0 ||
          value > 0xffffffff
        )
          throw Error("Invalid address list");
        return value;
      })
    : [];

function App() {
  const [lang, setLang] = useState(
    localStorage.getItem("silicon-language") === "ru" ? "ru" : "en",
  );
  const t = (en: string, ru: string) => (lang === "ru" ? ru : en);
  const [project, setProject] = useState<Project | null>(null);
  const current = useRef<Project | null>(null);
  const [view, setView] = useState<View>("circuit");
  const [wave, setWave] = useState<Wave | null>(null);
  const [ticks, setTicks] = useState(16);
  const [live, setLive] = useState(false);
  const [issue, setIssue] = useState("");
  const [status, setStatus] = useState("");
  const [assembly, setAssembly] = useState<Assembly | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [listing, setListing] = useState<string[]>([]);
  const [architecture, setArchitecture] = useState("");
  const [circuitSource, setCircuitSource] = useState("");
  const [breakpoints, setBreakpoints] = useState("");
  const [watchpoints, setWatchpoints] = useState("");
  const [address, setAddress] = useState("0");
  const [budget, setBudget] = useState(10000);
  const [running, setRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [palette, setPalette] = useState(false);
  const [query, setQuery] = useState("");
  const [fallback, setFallback] = useState(false);
  const [epoch, setEpoch] = useState(0);
  const [editCount, setEditCount] = useState(0);
  const past = useRef<Circuit[]>([]);
  const future = useRef<Circuit[]>([]);
  const cancel = useRef(false);
  const operation = useRef(false);
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const executing = useRef<{
    architecture: Project["architecture"];
    source: string;
  } | null>(null);
  const replace = useCallback((p: Project) => {
    cancel.current = true;
    executing.current = null;
    current.current = p;
    setProject(p);
    setArchitecture(JSON.stringify(p.architecture, null, 2));
    setCircuitSource(JSON.stringify(p.circuit, null, 2));
    setSnapshot(null);
    setAssembly(null);
    setWave(null);
    setListing([]);
    setEpoch((e) => e + 1);
    setEditCount(0);
    past.current = [];
    future.current = [];
  }, []);
  useEffect(() => {
    invoke<Project>("template", { name: "micro-8" })
      .then(replace)
      .catch((e) => setIssue(String(e)));
  }, [replace]);
  useEffect(() => {
    localStorage.setItem("silicon-language", lang);
    document.documentElement.lang = lang;
  }, [lang]);
  useEffect(() => {
    const model = editor.current?.getModel();
    if (!model || view !== "source") return;
    const match = /^(\d+):(\d+): (.+)$/.exec(issue);
    monaco.editor.setModelMarkers(
      model,
      "silicon",
      match && Number(match[1]) <= model.getLineCount()
        ? [
            {
              severity: monaco.MarkerSeverity.Error,
              startLineNumber: Number(match[1]),
              endLineNumber: Number(match[1]),
              startColumn: Number(match[2]),
              endColumn: Math.max(2, model.getLineMaxColumn(Number(match[1]))),
              message: match[3],
            },
          ]
        : [],
    );
  }, [issue, view]);
  const change = (p: Project) => {
    if (
      current.current?.source !== p.source ||
      current.current?.architecture !== p.architecture
    ) {
      executing.current = null;
      cancel.current = true;
      setAssembly(null);
      setSnapshot(null);
      setListing([]);
    }
    if (current.current?.circuit !== p.circuit) {
      setWave(null);
      setCircuitSource(JSON.stringify(p.circuit, null, 2));
    }
    current.current = p;
    setProject(p);
  };
  const commitCircuit = (c: Circuit) => {
    if (!project) return;
    past.current = [...past.current.slice(-39), project.circuit];
    future.current = [];
    change({ ...project, circuit: c });
    setEditCount((x) => x + 1);
  };
  const history = (redo: boolean) => {
    if (!project) return;
    const from = redo ? future : past,
      to = redo ? past : future;
    const next = from.current.pop();
    if (next) {
      to.current.push(project.circuit);
      change({ ...project, circuit: next });
      setEditCount((x) => x + 1);
    }
  };
  const job = async (fn: () => Promise<void>) => {
    if (operation.current) return;
    operation.current = true;
    setBusy(true);
    setIssue("");
    setStatus("");
    try {
      await fn();
    } catch (e) {
      setIssue(String(e instanceof Error ? e.message : e));
    } finally {
      operation.current = false;
      setBusy(false);
    }
  };
  const readProject = () => {
    if (!current.current) throw Error("Open a project first");
    return current.current;
  };
  const serialize = () => JSON.stringify(readProject(), null, 2);
  const open = () =>
    job(async () => {
      const p = await invoke<Project | null>("open_project");
      if (p) {
        replace(p);
        setStatus(t("Project opened", "Проект открыт"));
      }
    });
  const save = () =>
    job(async () => {
      if (await invoke<boolean>("save_project", { project: serialize() }))
        setStatus(t("Project saved", "Проект сохранён"));
    });
  const load = (name: string) =>
    job(async () => {
      if (!name) return;
      replace(await invoke<Project>("template", { name }));
      setStatus(t("Reference loaded", "Пример загружен"));
    });
  const assemble = () =>
    job(async () => {
      setView("source");
      const result = await invoke<Assembly>("assemble", {
        project: serialize(),
      });
      setAssembly(result);
      setStatus(
        t(
          `Assembled ${result.binary.length} bytes`,
          `Собрано байт: ${result.binary.length}`,
        ),
      );
    });
  const simulate = () =>
    job(async () => {
      const p = readProject();
      const result = await invoke<Wave>("simulate", {
        project: JSON.stringify(p),
        ticks,
      });
      if (p.circuit === current.current?.circuit) setWave(result);
      setStatus(
        t(
          `${result.events.length} transitions · ${result.conflicts.length} conflicts`,
          `Переходов: ${result.events.length} · конфликтов: ${result.conflicts.length}`,
        ),
      );
    });
  useEffect(() => {
    if (!live || !project || view !== "circuit") return;
    let active = true;
    const delay = setTimeout(() => {
      invoke<Wave>("simulate", { project: JSON.stringify(project), ticks })
        .then((result) => {
          if (active) {
            setWave(result);
            setIssue("");
          }
        })
        .catch((error) => {
          if (active) setIssue(String(error));
        });
    }, 220);
    return () => {
      active = false;
      clearTimeout(delay);
    };
  }, [live, project, ticks, view]);
  const debug = async (action: DebugAction, count = 0) => {
    const base = Number(address);
    if (!Number.isInteger(base) || base < 0 || base > 0xffffffff)
      throw Error("Invalid memory address");
    const result = await invoke<Snapshot>("debugger", {
      action,
      budget: count,
      breakpoints: numericList(breakpoints),
      watchpoints: numericList(watchpoints),
      address: base,
    });
    if (executing.current) setSnapshot(result);
    return result;
  };
  const prepare = async (force = false) => {
    const p = readProject();
    if (
      force ||
      executing.current?.architecture !== p.architecture ||
      executing.current?.source !== p.source
    ) {
      const lines = await invoke<string[]>("prepare", {
        project: JSON.stringify(p),
      });
      if (
        p.architecture !== current.current?.architecture ||
        p.source !== current.current?.source
      )
        throw Error("Program edited during preparation");
      executing.current = { architecture: p.architecture, source: p.source };
      setListing(lines);
    }
  };
  const single = (action: DebugAction, reset = false) =>
    job(async () => {
      setView("debugger");
      await prepare(reset);
      await debug(action);
    });
  const run = () =>
    job(async () => {
      setView("debugger");
      if (!Number.isInteger(budget) || budget < 1 || budget > 10_000_000)
        throw Error("Budget must be in 1…10000000");
      cancel.current = false;
      setRunning(true);
      try {
        await prepare();
        let remaining = budget;
        while (remaining > 0 && !cancel.current) {
          const batch = Math.min(remaining, 256);
          const result = await debug("run", batch);
          if (result.stop !== "budget") break;
          remaining -= batch;
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
        if (cancel.current) setStatus(t("Paused", "На паузе"));
      } finally {
        setRunning(false);
      }
    });
  const exportVcd = () =>
    job(async () => {
      if (await invoke<boolean>("export_vcd", { project: serialize(), ticks }))
        setStatus(t("VCD exported", "VCD экспортирован"));
    });
  const commands = [
    [t("Open project", "Открыть проект"), open],
    [t("Save project", "Сохранить проект"), save],
    [t("Assemble", "Ассемблировать"), assemble],
    [t("Run program", "Запустить программу"), run],
    [t("Step instruction", "Шаг инструкции"), () => single("step")],
    [t("Reverse instruction", "Шаг назад"), () => single("reverse")],
    [t("Simulate circuit", "Моделировать схему"), simulate],
    [t("Export VCD", "Экспорт VCD"), exportVcd],
  ] as const;
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setQuery("");
        setPalette((x) => !x);
      }
      if (mod && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void save();
      }
      if (e.key === "F5") {
        e.preventDefault();
        if (running) cancel.current = true;
        else void run();
      }
      if (e.key === "F10") {
        e.preventDefault();
        void single("step");
      }
      if (e.key === "Escape") setPalette(false);
      if (
        view === "circuit" &&
        mod &&
        e.key.toLowerCase() === "z" &&
        (e.target instanceof HTMLCanvasElement || e.target === document.body)
      ) {
        e.preventDefault();
        history(e.shiftKey);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
  const tabs: [View, string][] = [
    ["circuit", t("Schematic", "Схема")],
    ["source", t("Assembly", "Ассемблер")],
    ["architecture", t("ISA Workshop", "Редактор ISA")],
    ["debugger", t("Execution Lab", "Отладчик")],
    ["signals", t("Signal Analyzer", "Сигналы")],
  ];
  const state = snapshot?.state;
  const memoryRows = (data: number[], base: number) =>
    Array.from({ length: Math.ceil(data.length / 16) }, (_, i) => (
      <div className="hex-row" key={i}>
        <span>{hex(base + 16 * i, 8)}</span>{" "}
        {data
          .slice(i * 16, i * 16 + 16)
          .map((x) => hex(x, 2))
          .join(" ")}
      </div>
    ));
  return (
    <div className="studio">
      <header className="topbar">
        <div className="identity">
          <span className="chip">◈</span>
          <div>
            <strong>SILICON</strong>
            <small>
              {t("Architecture Development Studio", "Студия архитектур")}
            </small>
          </div>
        </div>
        <div className="top-actions">
          <button onClick={open} disabled={busy}>
            {t("Open", "Открыть")}
          </button>
          <button onClick={save} disabled={busy || !project}>
            {t("Save", "Сохранить")}
          </button>
          <button
            onClick={() => {
              setQuery("");
              setPalette(true);
            }}
          >
            {t("Commands", "Команды")} <kbd>Ctrl K</kbd>
          </button>
          <button
            aria-label={t("Switch language", "Сменить язык")}
            onClick={() => setLang(lang === "en" ? "ru" : "en")}
          >
            {lang === "en" ? "EN" : "RU"}
          </button>
        </div>
      </header>
      <div className="projectbar">
        <div className="project-title">
          <span className="status-dot" />
          <strong>{project?.name ?? "—"}</strong>
          <span>{project?.architecture.name ?? ""}</span>
        </div>
        <select
          aria-label={t("Reference project", "Пример проекта")}
          value=""
          disabled={busy}
          onChange={(e) => void load(e.target.value)}
        >
          <option value="">{t("Load reference…", "Загрузить пример…")}</option>
          {references.map((r) => (
            <option key={r} value={r}>
              {r.toUpperCase()}
            </option>
          ))}
        </select>
      </div>
      <nav aria-label={t("Workspace", "Рабочая область")}>
        {tabs.map(([id, name]) => (
          <button
            className={view === id ? "active" : ""}
            aria-current={view === id ? "page" : undefined}
            key={id}
            onClick={() => setView(id)}
          >
            {name}
          </button>
        ))}
      </nav>
      <main key={epoch}>
        {!project ? (
          <div className="empty">
            {t("Opening workspace…", "Открытие рабочей области…")}
          </div>
        ) : (
          <>
            {(view === "circuit" || view === "signals") && (
              <div className="tools toolbar">
                <button
                  className="primary"
                  onClick={() => void simulate()}
                  disabled={busy}
                >
                  {t("Simulate", "Моделировать")}
                </button>
                <label>
                  {t("Ticks", "Такты")}{" "}
                  <input
                    type="number"
                    min={1}
                    max={100000}
                    value={ticks}
                    onChange={(e) => setTicks(Number(e.target.value))}
                  />
                </label>
                {view === "signals" ? (
                  <button disabled={busy} onClick={exportVcd}>
                    {t("Export VCD", "Экспорт VCD")}
                  </button>
                ) : (
                  <>
                    <label>
                      <input
                        type="checkbox"
                        checked={live}
                        onChange={(e) => setLive(e.target.checked)}
                      />
                      {t("Live", "Авто")}
                    </label>
                    <button
                      disabled={!past.current.length}
                      onClick={() => history(false)}
                    >
                      {t("Undo", "Отменить")}
                    </button>
                    <button
                      disabled={!future.current.length}
                      onClick={() => history(true)}
                    >
                      {t("Redo", "Повторить")}
                    </button>
                    <button onClick={() => setView("signals")} disabled={!wave}>
                      {t("View signals", "Просмотр сигналов")}
                    </button>
                  </>
                )}
              </div>
            )}
            {view === "circuit" && (
              <Schematic
                circuit={project.circuit}
                update={commitCircuit}
                wave={wave}
                t={t}
              />
            )}
            {view === "signals" && (
              <Signals circuit={project.circuit} wave={wave} t={t} />
            )}
            {view === "source" && (
              <>
                <div className="tools toolbar">
                  <button
                    className="primary"
                    disabled={busy}
                    onClick={assemble}
                  >
                    {t("Assemble", "Ассемблировать")}
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => single("inspect", true)}
                  >
                    {t("Load debugger", "Открыть отладчик")}
                  </button>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={fallback}
                      onChange={(e) => setFallback(e.target.checked)}
                    />
                    {t("Plain editor", "Простой редактор")}
                  </label>
                </div>
                <div className="editor-layout">
                  <section className="panel">
                    <h2>{t("Program source", "Исходный код")}</h2>
                    {fallback ? (
                      <textarea
                        className="codearea"
                        spellCheck={false}
                        value={project.source}
                        onChange={(e) =>
                          change({ ...readProject(), source: e.target.value })
                        }
                      />
                    ) : (
                      <Editor
                        language="silicon-asm"
                        theme="silicon"
                        height="100%"
                        value={project.source}
                        onChange={(value) =>
                          change({ ...readProject(), source: value ?? "" })
                        }
                        onMount={(instance) => {
                          editor.current = instance;
                          instance.onDidDispose(() => {
                            if (editor.current === instance)
                              editor.current = null;
                          });
                        }}
                        options={{
                          minimap: { enabled: false },
                          fontSize: 14,
                          automaticLayout: true,
                          scrollBeyondLastLine: false,
                        }}
                      />
                    )}
                  </section>
                  <aside className="panel output">
                    <h2>{t("Assembly output", "Результат сборки")}</h2>
                    {assembly ? (
                      <>
                        <p>
                          {assembly.binary.length} {t("bytes", "байт")}
                        </p>
                        <pre>
                          {assembly.binary
                            .slice(0, 4096)
                            .map((x) => hex(x, 2))
                            .join(" ")}
                        </pre>
                        <h2>{t("Symbols", "Символы")}</h2>
                        <pre>{JSON.stringify(assembly.symbols, null, 2)}</pre>
                      </>
                    ) : (
                      <p>
                        {t(
                          "Assemble the source to inspect its bytes and symbols.",
                          "Соберите программу для просмотра байтов и символов.",
                        )}
                      </p>
                    )}
                  </aside>
                </div>
              </>
            )}
            {view === "architecture" && (
              <>
                <div className="tools toolbar">
                  <button
                    className="primary"
                    disabled={busy}
                    onClick={() =>
                      job(async () => {
                        const p = {
                          ...readProject(),
                          architecture: JSON.parse(architecture),
                        };
                        await invoke("assemble", {
                          project: JSON.stringify({ ...p, source: "" }),
                        });
                        change(p);
                        setStatus(
                          t("Architecture applied", "Архитектура применена"),
                        );
                      })
                    }
                  >
                    {t("Validate and apply ISA", "Проверить и применить ISA")}
                  </button>
                  <button
                    disabled={busy}
                    onClick={() =>
                      job(async () => {
                        const circuit: Circuit = JSON.parse(circuitSource);
                        await invoke("simulate", {
                          project: JSON.stringify({
                            ...readProject(),
                            circuit,
                          }),
                          ticks: 1,
                        });
                        commitCircuit(circuit);
                        setStatus(t("Circuit applied", "Схема применена"));
                      })
                    }
                  >
                    {t(
                      "Validate and apply circuit",
                      "Проверить и применить схему",
                    )}
                  </button>
                </div>
                <div className="editor-layout equal">
                  <section className="panel">
                    <h2>{t("Architecture JSON", "Архитектура JSON")}</h2>
                    <textarea
                      className="codearea"
                      spellCheck={false}
                      value={architecture}
                      onChange={(e) => setArchitecture(e.target.value)}
                    />
                  </section>
                  <section className="panel">
                    <h2>{t("Circuit JSON", "Схема JSON")}</h2>
                    <textarea
                      className="codearea"
                      spellCheck={false}
                      value={circuitSource}
                      onChange={(e) => setCircuitSource(e.target.value)}
                    />
                  </section>
                </div>
              </>
            )}
            {view === "debugger" && (
              <>
                <div className="tools toolbar">
                  <button className="primary" disabled={busy} onClick={run}>
                    {t("Run", "Пуск")} <kbd>F5</kbd>
                  </button>
                  <button
                    disabled={!running}
                    onClick={() => {
                      cancel.current = true;
                    }}
                  >
                    {t("Pause", "Пауза")}
                  </button>
                  <button disabled={busy} onClick={() => single("step")}>
                    {t("Step", "Шаг")} <kbd>F10</kbd>
                  </button>
                  <button disabled={busy} onClick={() => single("reverse")}>
                    {t("Reverse", "Назад")}
                  </button>
                  <button
                    disabled={busy}
                    onClick={() => single("inspect", true)}
                  >
                    {t("Reset", "Сброс")}
                  </button>
                  <label>
                    {t("Budget", "Лимит")}{" "}
                    <input
                      type="number"
                      min={1}
                      max={10000000}
                      value={budget}
                      onChange={(e) => setBudget(Number(e.target.value))}
                    />
                  </label>
                </div>
                <div className="debug-grid">
                  <section className="panel output">
                    <h2>{t("Registers", "Регистры")}</h2>
                    {state ? (
                      <div className="registers">
                        <code>
                          PC <strong>{hex(state.pc, 8)}</strong>
                        </code>
                        <code>
                          SP{" "}
                          <strong>
                            {state.sp === null ? "—" : hex(state.sp, 8)}
                          </strong>
                        </code>
                        {state.registers.map((r, i) => (
                          <code key={i}>
                            R{i} <strong>{hex(r, 8)}</strong>
                          </code>
                        ))}
                        {Object.entries(state.flags).map(([flag, v]) => (
                          <code key={flag}>
                            {flag.toUpperCase()} <strong>{Number(v)}</strong>
                          </code>
                        ))}
                        <code>
                          {t("CYCLES", "ТАКТЫ")} <strong>{state.cycles}</strong>
                        </code>
                      </div>
                    ) : (
                      <p>
                        {t(
                          "Run or step to load the program.",
                          "Запустите программу или выполните шаг.",
                        )}
                      </p>
                    )}
                    <h2>{t("Debug controls", "Параметры отладки")}</h2>
                    <label>
                      {t("Breakpoints", "Точки останова")}
                      <input
                        value={breakpoints}
                        placeholder="0x10, 0x20"
                        onChange={(e) => setBreakpoints(e.target.value)}
                      />
                    </label>
                    <label>
                      {t("Watchpoints", "Точки наблюдения")}
                      <input
                        value={watchpoints}
                        placeholder="0xF0"
                        onChange={(e) => setWatchpoints(e.target.value)}
                      />
                    </label>
                  </section>
                  <section className="panel output">
                    <h2>{t("Disassembly", "Дизассемблер")}</h2>
                    <div className="listing">
                      {listing.slice(0, 8192).map((line, i) => {
                        const pc =
                          Number(project.architecture.entry) +
                          i * Number(project.architecture.instruction_bytes);
                        return (
                          <button
                            key={i}
                            className={state?.pc === pc ? "pc" : ""}
                            onClick={() =>
                              setBreakpoints((value) => {
                                try {
                                  const values = numericList(value);
                                  return (
                                    values.includes(pc)
                                      ? values.filter((x) => x !== pc)
                                      : [...values, pc]
                                  )
                                    .map((x) => "0x" + hex(x))
                                    .join(", ");
                                } catch {
                                  return value;
                                }
                              })
                            }
                          >
                            <span>{hex(pc, 8)}</span> {line}
                          </button>
                        );
                      })}
                    </div>
                    <h2>{t("Instruction trace", "Трассировка")}</h2>
                    <pre>
                      {snapshot?.trace
                        .map(
                          (r) =>
                            `${r.cycle.toString().padStart(5)}  ${hex(r.pc, 8)}  ${r.instruction}`,
                        )
                        .join("\n")}
                    </pre>
                  </section>
                  <section className="panel output">
                    <h2>{t("Memory", "Память")}</h2>
                    <div className="tools">
                      <input
                        aria-label={t("Memory address", "Адрес памяти")}
                        value={address}
                        onChange={(e) => setAddress(e.target.value)}
                      />
                      <button disabled={busy} onClick={() => single("inspect")}>
                        {t("Go", "Перейти")}
                      </button>
                    </div>
                    <div className="hex">
                      {snapshot &&
                        memoryRows(snapshot.memory, snapshot.memory_base)}
                    </div>
                    <h2>{t("Stack", "Стек")}</h2>
                    <div className="hex">
                      {snapshot?.state.sp != null
                        ? memoryRows(snapshot.stack, snapshot.state.sp)
                        : "—"}
                    </div>
                    <h2>{t("Virtual console", "Виртуальная консоль")}</h2>
                    <pre className="console">{snapshot?.console ?? ""}</pre>
                  </section>
                </div>
              </>
            )}
          </>
        )}
      </main>
      <footer>
        <span className="status-dot" />
        <span
          className={issue ? "error" : ""}
          role={issue ? "alert" : "status"}
        >
          {issue ||
            status ||
            (busy
              ? t("Working…", "Выполняется…")
              : state?.halted
                ? t("Halted", "Остановлено")
                : (snapshot?.stop ?? t("Ready", "Готово")))}
        </span>
        <span className="foot-right">
          {project?.circuit.nodes.length ?? 0} {t("components", "компонентов")}{" "}
          · v0.1.0-alpha{editCount ? " · " + t("edited", "изменено") : ""}
        </span>
      </footer>
      {palette && (
        <div className="overlay" onClick={() => setPalette(false)}>
          <div
            className="palette panel"
            role="dialog"
            aria-label={t("Command palette", "Палитра команд")}
            onClick={(e) => e.stopPropagation()}
          >
            <input
              autoFocus
              placeholder={t("Search commands…", "Поиск команд…")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                const first = commands.find(([label]) =>
                  label.toLowerCase().includes(query.toLowerCase()),
                );
                if (e.key === "Enter" && first) {
                  setPalette(false);
                  void first[1]();
                }
              }}
            />
            {commands
              .filter(([label]) =>
                label.toLowerCase().includes(query.toLowerCase()),
              )
              .map(([label, action]) => (
                <button
                  key={label}
                  onClick={() => {
                    setPalette(false);
                    void action();
                  }}
                >
                  {label}
                </button>
              ))}
          </div>
        </div>
      )}
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
