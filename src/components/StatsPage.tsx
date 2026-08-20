import { formatPlaytime } from "../types";
import type { LibraryOverview } from "../types";
import { BarChart } from "./Charts";

interface Props {
  overview: LibraryOverview | null;
  loading: boolean;
  onOpenGame: (id: string) => void;
}

export function StatsPage({ overview, loading, onOpenGame }: Props) {
  if (loading || !overview) {
    return (
      <div className="stats-page">
        <h2>Library stats</h2>
        <p className="chart-empty">Crunching playtime…</p>
      </div>
    );
  }

  const y = overview.yearInReview;
  const monthly = y.monthly.map((m) => ({
    label: m.day.slice(5),
    value: Math.round(m.minutes / 60),
  }));

  return (
    <div className="stats-page">
      <h2>Library stats</h2>
      <div className="stats-hero-grid">
        <div className="stat-card">
          <span className="stat-label">This week</span>
          <span className="stat-value">{formatPlaytime(overview.minutesThisWeek)}</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">Play streak</span>
          <span className="stat-value">
            {overview.streakDays > 0 ? `${overview.streakDays} day${overview.streakDays === 1 ? "" : "s"}` : "—"}
          </span>
        </div>
        <div className="stat-card">
          <span className="stat-label">Most played</span>
          {overview.mostPlayed ? (
            <button type="button" className="linkish stat-link" onClick={() => onOpenGame(overview.mostPlayed!.gameId)}>
              {overview.mostPlayed.name}
              <span className="stat-sub">{formatPlaytime(overview.mostPlayed.minutes)}</span>
            </button>
          ) : (
            <span className="stat-value">—</span>
          )}
        </div>
        <div className="stat-card">
          <span className="stat-label">All-time</span>
          <span className="stat-value">{formatPlaytime(overview.totalPlaytimeMinutes)}</span>
          <span className="stat-sub">{overview.gamesPlayed} games played</span>
        </div>
      </div>

      <section className="year-review">
        <h3>{y.year} in review</h3>
        <p className="settings-lead">
          {formatPlaytime(y.totalMinutes)} across sessions started in this launcher.
        </p>
        <div className="charts-row">
          <BarChart title="Hours by month" points={monthly} formatValue={(v) => `${v}h`} />
          <div className="chart-card">
            <h3>Top games this year</h3>
            {y.topGames.length === 0 ? (
              <p className="chart-empty">No sessions logged this year yet.</p>
            ) : (
              <ol className="top-games">
                {y.topGames.map((g) => (
                  <li key={g.gameId}>
                    <button type="button" onClick={() => onOpenGame(g.gameId)}>
                      {g.name}
                    </button>
                    <span>{formatPlaytime(g.minutes)}</span>
                  </li>
                ))}
              </ol>
            )}
          </div>
        </div>
      </section>
    </div>
  );
}
