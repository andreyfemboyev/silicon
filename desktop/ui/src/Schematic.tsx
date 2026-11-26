import { useEffect, useRef, useState } from "react";
import type { PointerEvent } from "react";
import { library, ports, position } from "./model";
import type { Circuit, Node, T, Wave } from "./model";

const nodeWidth = 120;
const disconnected = 0xffffffff;
const nodeHeight = (node: Node) =>
  Math.max(55, 28 + ports(node.component).length * 15);
const snap = (v: number) => Math.round(v / 10) * 10;

export function Schematic({
  circuit,
  update,
  wave,
  t,
}: {
  circuit: Circuit;
  update: (c: Circuit) => void;
  wave: Wave | null;
  t: T;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 800, height: 460 });
  const [view, setView] = useState({ x: 0, y: 0, scale: 1 });
  const [selected, select] = useState<number | null>(null);
  const [wire, setWire] = useState<number | null>(null);
  const [draft, setDraft] = useState<{
    i: number;
    x: number;
    y: number;
  } | null>(null);
  const [fallback, setFallback] = useState(false);
  const drag = useRef<{
    i: number;
    x: number;
    y: number;
    startX: number;
    startY: number;
  } | null>(null);
  const copied = useRef<Node | null>(null);
  const [parameter, setParameter] = useState("");
  const nodes = circuit.nodes;
  const node = selected === null ? undefined : nodes[selected];
  const valueByNode: Record<number, string> = {};
  if (wave) for (const e of wave.events) valueByNode[e.node] = e.value;
  const change = (index: number, item: Node) =>
    update({
      ...circuit,
      nodes: nodes.map((n, i) => (i === index ? item : n)),
    });
  const locate = (n: Node, index: number) =>
    draft?.i === index ? draft : position(n, index);
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setSize({
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      }),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) {
      setFallback(true);
      return;
    }
    const ratio = Math.min(window.devicePixelRatio, 2);
    c.width = Math.max(1, Math.round(size.width * ratio));
    c.height = Math.max(1, Math.round(size.height * ratio));
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.fillStyle = "#101518";
    ctx.fillRect(0, 0, size.width, size.height);
    const grid = 20 * view.scale;
    ctx.fillStyle = "#283438";
    for (let x = ((view.x % grid) + grid) % grid; x < size.width; x += grid)
      for (let y = ((view.y % grid) + grid) % grid; y < size.height; y += grid)
        ctx.fillRect(x, y, 1, 1);
    ctx.translate(view.x, view.y);
    ctx.scale(view.scale, view.scale);
    nodes.forEach((n, i) => {
      const p = locate(n, i);
      n.inputs.forEach((parent, j) => {
        if (!nodes[parent]) return;
        const q = locate(nodes[parent], parent);
        const sx = q.x + nodeWidth,
          sy = q.y + nodeHeight(nodes[parent]) / 2,
          dx = p.x,
          dy = p.y + 29 + j * 15;
        const elbow = sx + Math.max(17, (dx - sx) / 2);
        const value = valueByNode[parent];
        ctx.strokeStyle =
          value === "1"
            ? "#82f5c2"
            : value?.includes("x")
              ? "#d89e6d"
              : value
                ? "#5bb8ca"
                : "#52636c";
        ctx.lineWidth = nodes[parent].width > 1 ? 2 : 1;
        ctx.beginPath();
        ctx.moveTo(sx, sy);
        ctx.lineTo(elbow, sy);
        ctx.lineTo(elbow, dy);
        ctx.lineTo(dx, dy);
        ctx.stroke();
      });
    });
    nodes.forEach((n, i) => {
      const p = locate(n, i);
      ctx.fillStyle = i === selected ? "#20332f" : "#1b252a";
      ctx.strokeStyle = i === selected || i === wire ? "#7ef5be" : "#41545d";
      ctx.lineWidth = 1;
      ctx.fillRect(p.x, p.y, nodeWidth, nodeHeight(n));
      ctx.strokeRect(p.x + 0.5, p.y + 0.5, nodeWidth, nodeHeight(n));
      ctx.font = "12px monospace";
      ctx.fillStyle = "#e1efec";
      ctx.fillText(n.name.slice(0, 15), p.x + 9, p.y + 16);
      ctx.font = "10px monospace";
      ctx.fillStyle = "#a6b8c0";
      ctx.fillText(
        n.component.kind.toUpperCase().slice(0, 18),
        p.x + 9,
        p.y + nodeHeight(n) - 8,
      );
      ports(n.component).forEach((_, j) => {
        ctx.fillStyle = "#78adbd";
        ctx.fillRect(p.x - 4, p.y + 26 + j * 15, 7, 7);
      });
      ctx.fillStyle = "#85edbd";
      ctx.fillRect(p.x + nodeWidth - 3, p.y + nodeHeight(n) / 2 - 3, 7, 7);
      ctx.fillText(
        valueByNode[i]?.slice(0, 14) ?? `${n.width}b`,
        p.x + nodeWidth + 9,
        p.y + nodeHeight(n) / 2 + 4,
      );
    });
  });
  const coord = (event: { clientX: number; clientY: number }) => {
    const rect = viewport.current!.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left - view.x) / view.scale,
      y: (event.clientY - rect.top - view.y) / view.scale,
    };
  };
  const add = (kind: string, x = 100, y = 100) => {
    if (!library.includes(kind) || nodes.length >= 4096) return;
    const component =
      kind === "input"
        ? { kind, value: "0" }
        : kind === "clock"
          ? { kind, period: 2 }
          : kind === "rom"
            ? { kind, words: [0, 1] }
            : kind === "ram"
              ? { kind, address_bits: 1 }
              : kind === "slice"
                ? { kind, lsb: 0 }
                : kind === "subcircuit"
                  ? {
                      kind,
                      ports: [0],
                      output: 1,
                      circuit: {
                        version: 1,
                        nodes: [
                          {
                            name: "input",
                            width: 1,
                            component: { kind: "input", value: "0" },
                            inputs: [],
                          },
                          {
                            name: "invert",
                            width: 1,
                            component: { kind: "not" },
                            inputs: [0],
                          },
                        ],
                      },
                    }
                  : { kind };
    let suffix = nodes.length;
    while (nodes.some((n) => n.name === `${kind}_${suffix}`)) suffix++;
    select(nodes.length);
    update({
      ...circuit,
      nodes: [
        ...nodes,
        {
          name: `${kind}_${suffix}`,
          width: 1,
          component,
          inputs: [],
          x: snap(x),
          y: snap(y),
        },
      ],
    });
  };
  const pointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    event.currentTarget.focus();
    const p = coord(event);
    for (let i = nodes.length - 1; i >= 0; i--) {
      const n = nodes[i],
        q = position(n, i);
      if (
        Math.abs(p.x - q.x - nodeWidth) < 9 &&
        Math.abs(p.y - q.y - nodeHeight(n) / 2) < 10
      ) {
        setWire(i);
        return;
      }
      const port = Math.round((p.y - q.y - 29) / 15);
      if (
        wire !== null &&
        Math.abs(p.x - q.x) < 9 &&
        port >= 0 &&
        port < ports(n.component).length &&
        nodes[wire]
      ) {
        const inputs = [...n.inputs];
        while (inputs.length <= port) inputs.push(disconnected);
        inputs[port] = wire;
        change(i, { ...n, inputs });
        setWire(null);
        return;
      }
      if (
        p.x >= q.x &&
        p.x <= q.x + nodeWidth &&
        p.y >= q.y &&
        p.y <= q.y + nodeHeight(n)
      ) {
        select(i);
        drag.current = {
          i,
          x: q.x,
          y: q.y,
          startX: event.clientX,
          startY: event.clientY,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
        return;
      }
    }
    select(null);
    setWire(null);
    drag.current = {
      i: -1,
      x: view.x,
      y: view.y,
      startX: event.clientX,
      startY: event.clientY,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const erase = () => {
    if (selected === null || !node) return;
    update({
      ...circuit,
      nodes: nodes
        .filter((_, i) => i !== selected)
        .map((n) => ({
          ...n,
          inputs: n.inputs.map((i) =>
            i === selected ? disconnected : i > selected ? i - 1 : i,
          ),
        })),
    });
    select(null);
    setWire(null);
  };
  const paste = () => {
    if (!copied.current || nodes.length >= 4096) return;
    const original = copied.current;
    let name = original.name + "_copy";
    while (nodes.some((n) => n.name === name)) name += "_";
    update({
      ...circuit,
      nodes: [
        ...nodes,
        {
          ...original,
          name,
          inputs: [],
          x: (original.x ?? 100) + 30,
          y: (original.y ?? 100) + 30,
        },
      ],
    });
    select(nodes.length);
  };
  useEffect(() => {
    setParameter(JSON.stringify(node?.component ?? {}, null, 2));
  }, [node]);
  const zoom = (factor: number) =>
    setView((v) => ({
      ...v,
      scale: Math.max(0.35, Math.min(3, v.scale * factor)),
    }));
  return (
    <div className="schematic">
      <aside className="library panel">
        <h2>{t("Components", "Компоненты")}</h2>
        {library.map((kind) => (
          <button
            key={kind}
            draggable
            onDragStart={(e) => e.dataTransfer.setData("text/plain", kind)}
            onClick={() => add(kind)}
          >
            <span className="pin" /> {kind.replaceAll("_", " ")}
          </button>
        ))}
      </aside>
      <div className="schematic-center panel">
        <div className="tools">
          <button onClick={() => zoom(1.2)}>+</button>
          <button onClick={() => zoom(1 / 1.2)}>−</button>
          <button onClick={() => setView({ x: 0, y: 0, scale: 1 })}>
            {Math.round(view.scale * 100)}%
          </button>
          <span>
            {wire === null
              ? t(
                  "Drag canvas to pan · Click an output, then an input to wire",
                  "Перетащите холст · Щёлкните выход, затем вход",
                )
              : t("Select an input pin", "Выберите вход")}
          </span>
        </div>
        <div
          className="viewport"
          ref={viewport}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            const p = coord(e);
            add(e.dataTransfer.getData("text/plain"), p.x, p.y);
          }}
        >
          {fallback ? (
            <div className="fallback">
              {nodes.map((n, i) => (
                <button key={i} onClick={() => select(i)}>
                  {n.name} · {valueByNode[i] ?? n.width}
                </button>
              ))}
            </div>
          ) : (
            <canvas
              aria-label={t("Circuit schematic", "Схема")}
              tabIndex={0}
              onPointerDown={pointerDown}
              onPointerCancel={() => {
                drag.current = null;
                setDraft(null);
              }}
              onPointerMove={(e) => {
                const d = drag.current;
                if (!d) return;
                const dx = e.clientX - d.startX,
                  dy = e.clientY - d.startY;
                if (d.i < 0)
                  setView((v) => ({ ...v, x: d.x + dx, y: d.y + dy }));
                else
                  setDraft({
                    i: d.i,
                    x: snap(d.x + dx / view.scale),
                    y: snap(d.y + dy / view.scale),
                  });
              }}
              onPointerUp={() => {
                if (draft && nodes[draft.i])
                  change(draft.i, {
                    ...nodes[draft.i],
                    x: draft.x,
                    y: draft.y,
                  });
                drag.current = null;
                setDraft(null);
              }}
              onWheel={(e) => zoom(e.deltaY < 0 ? 1.1 : 1 / 1.1)}
              onKeyDown={(e) => {
                if (e.key === "Delete") erase();
                if ((e.ctrlKey || e.metaKey) && e.key === "c" && node) {
                  e.preventDefault();
                  copied.current = node;
                }
                if ((e.ctrlKey || e.metaKey) && e.key === "v") {
                  e.preventDefault();
                  paste();
                }
                if (e.key === "Escape") setWire(null);
              }}
            />
          )}
        </div>
      </div>
      <aside className="inspector panel">
        <h2>{t("Inspector", "Инспектор")}</h2>
        {node && selected !== null ? (
          <>
            <label>
              {t("Name", "Имя")}
              <input
                value={node.name}
                onChange={(e) =>
                  change(selected, { ...node, name: e.target.value })
                }
              />
            </label>
            <label>
              {t("Width", "Разрядность")}
              <input
                type="number"
                min={1}
                max={32}
                value={node.width}
                onChange={(e) =>
                  change(selected, { ...node, width: Number(e.target.value) })
                }
              />
            </label>
            {Array.from(
              {
                length: Math.max(
                  node.inputs.length,
                  ports(node.component).length,
                ),
              },
              (_, i) => i,
            ).map((i) => (
              <label key={i}>
                {ports(node.component)[i] ?? `IN ${i}`}
                <select
                  value={node.inputs[i] ?? disconnected}
                  onChange={(e) => {
                    const inputs = [...node.inputs];
                    while (inputs.length <= i) inputs.push(disconnected);
                    inputs[i] = Number(e.target.value);
                    change(selected, { ...node, inputs });
                  }}
                >
                  <option value={disconnected}>—</option>
                  {nodes.map((n, j) => (
                    <option value={j} key={j}>
                      {n.name}
                    </option>
                  ))}
                </select>
              </label>
            ))}
            {node.component.kind === "resolve" && node.inputs.length < 64 && (
              <button
                onClick={() =>
                  change(selected, {
                    ...node,
                    inputs: [...node.inputs, disconnected],
                  })
                }
              >
                {t("Add driver", "Добавить драйвер")}
              </button>
            )}
            <label>
              {t("Parameters (JSON)", "Параметры (JSON)")}
              <textarea
                rows={5}
                value={parameter}
                onChange={(e) => setParameter(e.target.value)}
              />
            </label>
            <button
              onClick={() => {
                try {
                  const component = JSON.parse(parameter);
                  if (!library.includes(component.kind))
                    throw Error("Unknown component");
                  change(selected, { ...node, component });
                } catch (e) {
                  window.alert(String(e));
                }
              }}
            >
              {t("Apply parameters", "Применить")}
            </button>
            <div className="tools">
              <button
                onClick={() => {
                  copied.current = node;
                }}
              >
                {t("Copy", "Копировать")}
              </button>
              <button onClick={paste}>{t("Paste", "Вставить")}</button>
              <button onClick={erase}>{t("Delete", "Удалить")}</button>
            </div>
          </>
        ) : (
          <p>
            {t(
              "Select a component to edit its ports and parameters.",
              "Выберите компонент для настройки.",
            )}
          </p>
        )}
      </aside>
    </div>
  );
}
