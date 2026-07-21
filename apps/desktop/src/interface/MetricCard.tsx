import "./metricCard.css";

export interface MetricCardProps {
  title: string;
  value: string | number | null;
  subtitle?: string;
  busy?: boolean;
  error?: string | null;
}

/** Scalar KPI card for interface dashboards (DuckDB / SQLite aggregates). */
export function MetricCard({ title, value, subtitle, busy, error }: MetricCardProps) {
  let body: string;
  if (busy) body = "…";
  else if (error) body = "—";
  else if (value === null || value === undefined) body = "—";
  else body = String(value);

  return (
    <article className="lt-metric-card" aria-busy={busy || undefined}>
      <h3 className="lt-metric-card__title">{title}</h3>
      <p className="lt-metric-card__value">{body}</p>
      {subtitle ? <p className="lt-metric-card__subtitle">{subtitle}</p> : null}
      {error ? (
        <p className="lt-metric-card__error" role="alert">
          {error}
        </p>
      ) : null}
    </article>
  );
}
