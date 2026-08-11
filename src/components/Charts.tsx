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
