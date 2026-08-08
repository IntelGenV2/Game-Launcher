interface BarPoint {
  label: string;
  value: number;
}

interface Props {
  title: string;
  points: BarPoint[];
  color?: string;
  emptyText?: string;
  formatValue?: (v: number) => string;
}

export function BarChart({
  title,
  points,
  color = "#3ecf8e",
  emptyText = "No data yet — play to build this chart.",
  formatValue = (v) => String(Math.round(v)),
}: Props) {
  const max = Math.max(...points.map((p) => p.value), 1);
  const hasData = points.some((p) => p.value > 0);

  return (
    <div className="chart-card">
      <h3>{title}</h3>
      {!hasData ? (
        <p className="chart-empty">{emptyText}</p>
      ) : (
        <div className="bar-chart" role="img" aria-label={title}>
          {points.map((p) => (
            <div className="bar-col" key={p.label} title={`${p.label}: ${formatValue(p.value)}`}>
              <div className="bar-track">
                <div
                  className="bar-fill"
                  style={{
                    height: `${Math.max(4, (p.value / max) * 100)}%`,
                    background: color,
                  }}
                />
              </div>
              <span className="bar-label">{p.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface LineProps {
  title: string;
  points: { label: string; value: number }[];
  color?: string;
  emptyText?: string;
  unit?: string;
}

export function LineChart({
  title,
  points,
  color = "#5b9cff",
  emptyText = "Play the game to build an automatic FPS chart.",
  unit = "FPS",
}: LineProps) {
  const width = 480;
  const height = 160;
  const pad = 24;
  if (points.length === 0) {
    return (
      <div className="chart-card">
        <h3>{title}</h3>
        <p className="chart-empty">{emptyText}</p>
      </div>
    );
  }

  const max = Math.max(...points.map((p) => p.value), 1);
  const min = Math.min(...points.map((p) => p.value), 0);
  const span = Math.max(max - min, 1);
  const coords = points.map((p, i) => {
    const x = pad + (i / Math.max(points.length - 1, 1)) * (width - pad * 2);
    const y = height - pad - ((p.value - min) / span) * (height - pad * 2);
    return { x, y, ...p };
  });
  const path = coords.map((c, i) => `${i === 0 ? "M" : "L"}${c.x},${c.y}`).join(" ");

  return (
    <div className="chart-card">
      <h3>{title}</h3>
      <svg viewBox={`0 0 ${width} ${height}`} className="line-chart" role="img" aria-label={title}>
        <defs>
          <linearGradient id="fpsGlow" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity="0.35" />
            <stop offset="100%" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
        {coords.length > 1 && (
          <path
            d={`${path} L${coords[coords.length - 1].x},${height - pad} L${coords[0].x},${height - pad} Z`}
            fill="url(#fpsGlow)"
          />
        )}
        <path d={path} fill="none" stroke={color} strokeWidth="2.5" strokeLinecap="round" />
        {coords.map((c) => (
          <circle key={c.label + c.value} cx={c.x} cy={c.y} r="4" fill={color}>
            <title>{`${c.label}: ${Math.round(c.value)} ${unit}`}</title>
          </circle>
        ))}
      </svg>
      <div className="line-legend">
        <span>Avg {Math.round(points.reduce((a, p) => a + p.value, 0) / points.length)} {unit}</span>
        <span>Latest {Math.round(points[points.length - 1].value)} {unit}</span>
      </div>
    </div>
  );
}
