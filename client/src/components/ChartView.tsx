import { Download } from "lucide-react";
import { useRef, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip as ChartTooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { ChartSeries, ChartSpec } from "../api";
import { formatMoney, formatNumber, formatPercent, formatRatio } from "../format";

const COLORS = ["#10a37f", "#0866ff", "#f59e0b"];
const AXIS = "#6b6b76";
const GRID = "#e5e5e5";
const FONT = "Inter, -apple-system, Segoe UI, Roboto, Arial, sans-serif";
const compact = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 });

function formatValue(value: number, unit: ChartSeries["unit"], currency: string | null): string {
  if (unit === "money") return formatMoney(value, currency);
  if (unit === "percent") return formatPercent(value);
  if (unit === "ratio") return formatRatio(value);
  return formatNumber(value);
}

function formatTick(value: number, unit: ChartSeries["unit"]): string {
  if (unit === "percent") return `${(value * 100).toFixed(1)}%`;
  if (unit === "ratio") return `${value.toFixed(1)}x`;
  return compact.format(value);
}

function shortLabel(label: string, level: ChartSpec["level"]): string {
  if (level === "day") return label.slice(5);
  return label.length > 14 ? `${label.slice(0, 13)}…` : label;
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Could not render the chart image"));
    image.src = url;
  });
}

function toBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("Could not encode the PNG"))), "image/png");
  });
}

export function ChartView({ spec }: { spec: ChartSpec }) {
  const frame = useRef<HTMLDivElement>(null);
  const [note, setNote] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const units = Array.from(new Set(spec.series.map((item) => item.unit)));
  const axisOf = (unit: ChartSeries["unit"]) => (unit === units[0] ? "left" : "right");
  const unitOf = (key: string) => spec.series.find((item) => item.key === key)?.unit ?? "count";
  const dense = spec.data.length > 31;
  const rotate = spec.level !== "day" && spec.data.length > 4;

  const exportPng = async () => {
    const svg = frame.current?.querySelector("svg.recharts-surface");
    if (!svg || saving) return;
    setSaving(true);
    setNote(null);
    try {
      const box = svg.getBoundingClientRect();
      const clone = svg.cloneNode(true) as SVGSVGElement;
      clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
      clone.setAttribute("width", String(box.width));
      clone.setAttribute("height", String(box.height));
      clone.style.fontFamily = FONT;
      const xml = new XMLSerializer().serializeToString(clone);
      const image = await loadImage(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(xml)}`);
      const scale = 2;
      const header = 64;
      const canvas = document.createElement("canvas");
      canvas.width = Math.round(box.width * scale);
      canvas.height = Math.round((box.height + header) * scale);
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("Canvas is not available");
      ctx.scale(scale, scale);
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, box.width, box.height + header);
      ctx.fillStyle = "#0d0d0d";
      ctx.font = `600 15px ${FONT}`;
      ctx.textBaseline = "top";
      ctx.fillText(spec.title, 16, 14);
      ctx.fillStyle = AXIS;
      ctx.font = `12px ${FONT}`;
      const range = `${spec.start} to ${spec.end}${spec.currency ? ` · ${spec.currency}` : ""}`;
      ctx.fillText(range, 16, 34);
      let x = 16;
      spec.series.forEach((item, index) => {
        ctx.fillStyle = COLORS[index % COLORS.length];
        ctx.fillRect(x, 53, 10, 10);
        ctx.fillStyle = AXIS;
        ctx.fillText(item.name, x + 15, 51);
        x += 15 + ctx.measureText(item.name).width + 18;
      });
      ctx.drawImage(image, 0, header, box.width, box.height);
      const blob = await toBlob(canvas);
      const bytes = new Uint8Array(await blob.arrayBuffer());
      const stamp = new Date().toISOString().slice(0, 19).replace(/[-:T]/g, "");
      const base = spec.title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      const path = await window.heimdall.saveChartPng(`heimdall-${base || "chart"}-${stamp}`, Array.from(bytes));
      setNote({ tone: "ok", text: `Saved to ${path}` });
    } catch (failure) {
      setNote({ tone: "error", text: failure instanceof Error ? failure.message : String(failure) });
    } finally {
      setSaving(false);
    }
  };

  const axes = (
    <>
      <CartesianGrid stroke={GRID} strokeDasharray="3 3" vertical={false} />
      <XAxis
        dataKey="label"
        stroke={GRID}
        tickLine={false}
        interval={dense ? "preserveStartEnd" : 0}
        height={rotate ? 64 : 30}
        angle={rotate ? -30 : 0}
        textAnchor={rotate ? "end" : "middle"}
        tick={{ fill: AXIS, fontSize: 11 }}
        tickFormatter={(value: string) => shortLabel(String(value), spec.level)}
      />
      <YAxis
        yAxisId="left"
        stroke={GRID}
        tickLine={false}
        width={52}
        tick={{ fill: AXIS, fontSize: 11 }}
        tickFormatter={(value: number) => formatTick(value, units[0])}
      />
      {units.length > 1 && (
        <YAxis
          yAxisId="right"
          orientation="right"
          stroke={GRID}
          tickLine={false}
          width={52}
          tick={{ fill: AXIS, fontSize: 11 }}
          tickFormatter={(value: number) => formatTick(value, units[1])}
        />
      )}
      <ChartTooltip
        cursor={{ stroke: GRID }}
        contentStyle={{ borderRadius: 8, border: `1px solid ${GRID}`, fontSize: 12 }}
        formatter={(value, _name, item) => formatValue(Number(value), unitOf(String(item.dataKey)), spec.currency)}
      />
    </>
  );

  return (
    <figure className="chat-chart">
      <figcaption className="chart-head">
        <div>
          <div className="chart-title">{spec.title}</div>
          <div className="chart-meta">
            {spec.start} to {spec.end}
            {spec.currency ? ` · ${spec.currency}` : ""}
          </div>
        </div>
        <button
          type="button"
          className="btn chart-export"
          onClick={() => void exportPng()}
          disabled={saving}
          title="Save the chart as a PNG in your Downloads folder"
        >
          <Download size={14} /> PNG
        </button>
      </figcaption>
      <div className="chart-legend">
        {spec.series.map((item, index) => (
          <span key={item.key}>
            <i style={{ background: COLORS[index % COLORS.length] }} />
            {item.name}
          </span>
        ))}
      </div>
      <div className="chart-frame" ref={frame}>
        <ResponsiveContainer width="100%" height={rotate ? 300 : 260}>
          {spec.type === "bar" ? (
            <BarChart data={spec.data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
              {axes}
              {spec.series.map((item, index) => (
                <Bar
                  key={item.key}
                  dataKey={item.key}
                  name={item.name}
                  yAxisId={axisOf(item.unit)}
                  fill={COLORS[index % COLORS.length]}
                  radius={[3, 3, 0, 0]}
                  maxBarSize={36}
                  isAnimationActive={false}
                />
              ))}
            </BarChart>
          ) : (
            <LineChart data={spec.data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
              {axes}
              {spec.series.map((item, index) => (
                <Line
                  key={item.key}
                  type="monotone"
                  dataKey={item.key}
                  name={item.name}
                  yAxisId={axisOf(item.unit)}
                  stroke={COLORS[index % COLORS.length]}
                  strokeWidth={2}
                  dot={dense ? false : { r: 3 }}
                  connectNulls={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          )}
        </ResponsiveContainer>
      </div>
      {note && <p className={`chart-note ${note.tone}`}>{note.text}</p>}
    </figure>
  );
}
