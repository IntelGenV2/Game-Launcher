import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";

export interface SystemInfo {
  hostname: string;
  os: string;
  osVersion: string;
  cpu: string;
  cpuCores: number;
  ramTotalBytes: number;
  ramAvailableBytes: number;
  ramUsedBytes: number;
  gpu: string;
  display: string;
  monitors: number;
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return "—";
  const gb = bytes / 1024 ** 3;
  if (gb >= 10) return `${gb.toFixed(0)} GB`;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / 1024 ** 2;
  return `${mb.toFixed(0)} MB`;
}

function Row({
  label,
  value,
  extra,
}: {
  label: string;
  value: string;
  extra?: ReactNode;
}) {
  return (
    <div className="sys-row">
      <dt>{label}</dt>
      <dd>
        <span>{value || "—"}</span>
        {extra}
      </dd>
    </div>
  );
}

export function SystemPage({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const load = () => {
      invoke<SystemInfo>("system_info")
        .then((next) => {
          if (!cancelled) {
            setInfo(next);
            setError(null);
          }
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        });
    };
    load();
    const id = window.setInterval(load, 2500);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [open]);

  if (!open) return null;

  const used = info?.ramUsedBytes ?? 0;
  const usedPct =
    info && info.ramTotalBytes > 0 ? Math.min(100, (used / info.ramTotalBytes) * 100) : 0;

  return (
    <div className="settings-backdrop" onClick={onClose}>
      <div
        className="settings-panel sys-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="System"
      >
        <h2>System</h2>

        {error && !info ? (
          <p className="chart-empty">{error}</p>
        ) : !info ? (
          <p className="chart-empty">Reading specs…</p>
        ) : (
          <dl className="sys-facts">
            <Row label="OS" value={[info.os, info.osVersion].filter(Boolean).join(" ")} />
            <Row label="Processor" value={info.cpu} />
            <Row
              label="Cores"
              value={info.cpuCores ? `${info.cpuCores} logical` : "—"}
            />
            <Row label="Graphics" value={info.gpu} />
            <Row
              label="Memory"
              value={`${formatBytes(info.ramTotalBytes)}  ·  ${formatBytes(used)} in use`}
              extra={
                <div className="ram-meter" aria-hidden>
                  <div className="ram-meter-fill" style={{ width: `${usedPct}%` }} />
                </div>
              }
            />
            <Row
              label="Display"
              value={
                info.display
                  ? `${info.display}${info.monitors > 1 ? `  ·  ${info.monitors} monitors` : ""}`
                  : "—"
              }
            />
          </dl>
        )}
      </div>
    </div>
  );
}
