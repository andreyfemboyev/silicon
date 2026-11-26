import { useEffect, useMemo, useRef, useState } from "react";
import type { Circuit, T, Wave } from "./model";

const labelWidth = 154;
const rowHeight = 33;
const rulerHeight = 24;

function predecessor(events: Wave["events"], tick: number) {
  let low = 0,
    high = events.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (events[mid].tick <= tick) low = mid + 1;
    else high = mid;
  }
  return low - 1;
}

export function Signals({
  circuit,
  wave,
  t,
}: {
  circuit: Circuit;
  wave: Wave | null;
  t: T;
}) {
  const [zoom, setZoom] = useState(36);
  const [cursor, setCursor] = useState(0);
  const [trigger, setTrigger] = useState("");
  const [edge, setEdge] = useState("rise");
  const [scroll, setScroll] = useState({ x: 0, y: 0 });
  const [size, setSize] = useState({ width: 600, height: 320 });
  const [notice, setNotice] = useState("");
  const canvas = useRef<HTMLCanvasElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const ticks = wave?.ticks ?? 1;
  const at = Math.min(cursor, ticks - 1);
  const channel =
    circuit.nodes[Number(trigger)]?.width === 1 && trigger !== ""
      ? trigger
      : String(circuit.nodes.findIndex((node) => node.width === 1));
  const channels = useMemo(() => {
    const grouped = new Map<number, Wave["events"]>();
    for (const e of wave?.events ?? []) {
      if (!grouped.has(e.node)) grouped.set(e.node, []);
      grouped.get(e.node)!.push(e);
    }
    return grouped;
  }, [wave]);
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const observer = new ResizeObserver(() =>
      setSize({ width: element.clientWidth, height: element.clientHeight }),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [Boolean(wave)]);
  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx || !wave) return;
    const ratio = Math.min(devicePixelRatio, 2);
    canvas.current!.width = Math.max(1, Math.round(size.width * ratio));
    canvas.current!.height = Math.max(1, Math.round(size.height * ratio));
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    const { width, height } = size;
    ctx.fillStyle = "#101518";
    ctx.fillRect(0, 0, width, height);
    ctx.font = "11px monospace";
    const tickStart = scroll.x / zoom;
    const tickEnd = (scroll.x + width - labelWidth) / zoom;
    const spacing = Math.max(1, Math.ceil(45 / zoom));
    for (
      let tick = Math.floor(tickStart / spacing) * spacing;
      tick <= Math.min(wave.ticks, tickEnd + spacing);
      tick += spacing
    ) {
      const x = labelWidth + tick * zoom - scroll.x;
      ctx.fillStyle = "#29383f";
      ctx.fillRect(x, rulerHeight, 1, height);
      ctx.fillStyle = "#a5b9c3";
      ctx.fillText(String(tick), x + 3, 15);
    }
    const first = Math.max(0, Math.floor(scroll.y / rowHeight));
    const last = Math.min(
      circuit.nodes.length,
      Math.ceil((scroll.y + height) / rowHeight),
    );
    ctx.save();
    ctx.beginPath();
    ctx.rect(
      labelWidth,
      rulerHeight,
      Math.max(0, width - labelWidth),
      Math.max(0, height - rulerHeight),
    );
    ctx.clip();
    for (let i = first; i < last; i++) {
      const node = circuit.nodes[i];
      const y = rulerHeight + i * rowHeight - scroll.y;
      ctx.fillStyle = "#253138";
      ctx.fillRect(labelWidth, y + rowHeight - 1, width, 1);
      const events = channels.get(i) ?? [];
      const signalY = (value: string) =>
        y + (value === "0" ? 23 : value === "1" ? 6 : 15);
      for (
        let j = Math.max(0, predecessor(events, tickStart));
        j < events.length;
        j++
      ) {
        const e = events[j];
        if (e.tick > tickEnd) break;
        const x = labelWidth + e.tick * zoom - scroll.x,
          end =
            labelWidth + (events[j + 1]?.tick ?? wave.ticks) * zoom - scroll.x;
        const middle = signalY(e.value);
        ctx.strokeStyle = e.value.includes("x")
          ? "#dda673"
          : node.width === 1
            ? "#86efbf"
            : "#64c7dc";
        ctx.beginPath();
        ctx.moveTo(x, j > 0 ? signalY(events[j - 1].value) : middle);
        ctx.lineTo(x, middle);
        ctx.lineTo(end, middle);
        ctx.stroke();
        if (node.width > 1 && end - Math.max(labelWidth, x) > 30) {
          ctx.fillStyle = ctx.strokeStyle;
          const value = /^[01]+$/.test(e.value)
            ? parseInt(e.value, 2).toString(16).toUpperCase()
            : e.value;
          const start = Math.max(labelWidth, x) + 4;
          ctx.fillText(value, start, y + 12, Math.max(1, end - start - 4));
        }
      }
    }
    ctx.restore();
    const cursorX = labelWidth + at * zoom - scroll.x;
    if (cursorX >= labelWidth) {
      ctx.fillStyle = "#f1cf8c";
      ctx.fillRect(cursorX, 0, 1, height);
    }
    ctx.fillStyle = "#182226";
    ctx.fillRect(0, 0, labelWidth, height);
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, rulerHeight, labelWidth, Math.max(0, height - rulerHeight));
    ctx.clip();
    for (let i = first; i < last; i++) {
      const y = rulerHeight + i * rowHeight - scroll.y;
      ctx.fillStyle = "#afc1c9";
      ctx.fillText(circuit.nodes[i].name, 9, y + 20, labelWidth - 18);
      ctx.fillStyle = "#29383f";
      ctx.fillRect(0, y + rowHeight - 1, labelWidth, 1);
    }
    ctx.restore();
  }, [at, channels, circuit, scroll, size, wave, zoom]);
  const findTrigger = () => {
    const events = channels.get(Number(channel)) ?? [];
    const from = edge === "rise" ? "0" : "1",
      to = edge === "rise" ? "1" : "0";
    const matches = (event: Wave["events"][number], i: number) =>
      event.value === to && events[i - 1]?.value === from;
    const found =
      events.find((event, i) => event.tick > at && matches(event, i)) ??
      events.find(matches);
    if (found) {
      setCursor(found.tick);
      setNotice("");
      scroller.current?.scrollTo({
        left: Math.max(0, found.tick * zoom - 140),
      });
    } else setNotice(t("No matching edge", "Фронт не найден"));
  };
  if (!wave)
    return (
      <div className="empty">
        {t(
          "Simulate the circuit to capture signal transitions.",
          "Запустите моделирование для записи сигналов.",
        )}
      </div>
    );
  return (
    <div className="signal-pane">
      <div className="tools">
        <button
          aria-label={t("Zoom in", "Увеличить")}
          onClick={() => setZoom((z) => Math.min(96, z * 1.4))}
        >
          +
        </button>
        <button
          aria-label={t("Zoom out", "Уменьшить")}
          onClick={() => setZoom((z) => Math.max(0.5, z / 1.4))}
        >
          −
        </button>
        <label>
          {t("Cursor", "Курсор")}{" "}
          <input
            type="number"
            min={0}
            max={ticks - 1}
            value={at}
            onChange={(e) =>
              setCursor(
                Math.min(ticks - 1, Math.max(0, Number(e.target.value))),
              )
            }
          />
        </label>
        <select
          aria-label={t("Trigger channel", "Канал запуска")}
          value={channel}
          onChange={(e) => setTrigger(e.target.value)}
        >
          {channel === "-1" && <option value="-1">—</option>}
          {circuit.nodes.map(
            (n, i) =>
              n.width === 1 && (
                <option key={i} value={i}>
                  {n.name}
                </option>
              ),
          )}
        </select>
        <select
          aria-label={t("Trigger edge", "Фронт запуска")}
          value={edge}
          onChange={(e) => setEdge(e.target.value)}
        >
          <option value="rise">{t("Rising", "Передний фронт")}</option>
          <option value="fall">{t("Falling", "Задний фронт")}</option>
        </select>
        <button disabled={channel === "-1"} onClick={findTrigger}>
          {t("Find edge", "Найти фронт")}
        </button>
        <span role="status">{notice}</span>
      </div>
      <div
        className="wave-scroll"
        ref={scroller}
        onScroll={(e) =>
          setScroll({
            x: e.currentTarget.scrollLeft,
            y: e.currentTarget.scrollTop,
          })
        }
      >
        <div
          className="wave-content"
          style={{
            width: Math.max(size.width, labelWidth + ticks * zoom + 44),
            height: Math.max(
              size.height,
              rulerHeight + circuit.nodes.length * rowHeight,
            ),
          }}
        >
          <canvas
            aria-label={t("Signal waveforms", "Временные диаграммы")}
            style={{
              width: size.width,
              height: size.height,
              top: scroll.y,
              left: scroll.x,
            }}
            onClick={(e) => {
              const x =
                e.clientX -
                e.currentTarget.getBoundingClientRect().left -
                labelWidth;
              if (x >= 0)
                setCursor(
                  Math.max(
                    0,
                    Math.min(ticks - 1, Math.floor((x + scroll.x) / zoom)),
                  ),
                );
            }}
          />
        </div>
      </div>
      <div className="signals-values">
        {circuit.nodes.map((node, i) => {
          const events = channels.get(i) ?? [];
          return (
            <code key={i}>
              {node.name}{" "}
              <strong>{events[predecessor(events, at)]?.value ?? "—"}</strong>
            </code>
          );
        })}
      </div>
    </div>
  );
}
