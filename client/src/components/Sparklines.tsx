interface SparkProps {
  values: number[];
  color: string;
}

const WIDTH = 400;
const HEIGHT = 48;
const PAD = 4;

export function LineSpark({ values, color }: SparkProps) {
  const series = values.length > 1 ? values : [0, 0];
  const max = Math.max(...series, 0);
  const points = series.map((value, index) => {
    const x = (index / (series.length - 1)) * WIDTH;
    const y = max === 0 ? HEIGHT - PAD : PAD + (1 - value / max) * (HEIGHT - PAD * 2);
    return { x, y };
  });
  const last = points[points.length - 1];

  return (
    <div className="spark-line">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none" aria-hidden="true">
        <polyline
          points={points.map((point) => `${point.x},${point.y}`).join(" ")}
          fill="none"
          stroke={color}
          strokeWidth="2"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <span className="spark-dot" style={{ borderColor: color, top: `${(last.y / HEIGHT) * 100}%` }} />
    </div>
  );
}

export function BarSpark({ values, color }: SparkProps) {
  const max = Math.max(...values, 0);

  return (
    <div className="spark-bars" aria-hidden="true">
      {values.map((value, index) => (
        <span
          key={index}
          className={value > 0 ? "spark-bar on" : "spark-bar"}
          style={value > 0 ? { height: `${Math.max(10, (value / max) * 100)}%`, background: color } : undefined}
        />
      ))}
    </div>
  );
}
