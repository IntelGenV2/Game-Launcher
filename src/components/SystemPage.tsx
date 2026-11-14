import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";

export interface DiskInfo { name: string; mount: string; totalBytes: number; availableBytes: number }
interface Hardware {
  processors?: { Name: string; NumberOfCores: number; NumberOfLogicalProcessors: number; MaxClockSpeed: number; L3CacheSize: number }[];
  memory?: { DeviceLocator: string; Capacity: number; ConfiguredClockSpeed: number; Speed: number; Manufacturer: string; PartNumber: string }[];
  graphics?: { Name: string; DriverVersion: string; VideoProcessor: string; CurrentHorizontalResolution: number; CurrentVerticalResolution: number; CurrentRefreshRate: number }[];
  storage?: { FriendlyName: string; MediaType: string | number; BusType: string | number; HealthStatus: string | number; Size: number }[];
  board?: { Manufacturer: string; Product: string }[];
  bios?: { SMBIOSBIOSVersion: string }[];
  powerPlan?: string;
}
export interface SystemInfo {
  hostname: string; username: string; os: string; osVersion: string; kernel: string; architecture: string;
  cpu: string; cpuCores: number; cpuMhz: number; ramTotalBytes: number; ramAvailableBytes: number;
  ramUsedBytes: number; gpu: string; display: string; monitors: number; uptimeSeconds: number;
  disks: DiskInfo[]; hardware?: Hardware | null;
}
export const SYSTEM_SECTIONS = [
  { id: "overview", label: "Overview" }, { id: "processor", label: "Processor" },
  { id: "memory", label: "Memory" }, { id: "graphics", label: "Graphics" },
  { id: "displays", label: "Displays" }, { id: "storage", label: "Storage" },
  { id: "windows", label: "Windows" }, { id: "files", label: "Files" },
] as const;
export type SystemSection = typeof SYSTEM_SECTIONS[number]["id"];
export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const d = Math.floor(seconds / 86400), h = Math.floor(seconds % 86400 / 3600), m = Math.floor(seconds % 3600 / 60);
  return d ? `${d}d ${h}h ${m}m` : h ? `${h}h ${m}m` : `${m}m`;
}
function bytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "Unavailable";
  if (value === 0) return "0 GB";
  return value >= 1024 ** 3 ? `${(value / 1024 ** 3).toFixed(1)} GB` : `${Math.round(value / 1024 ** 2)} MB`;
}
function Row({ label, value, children }: { label: string; value?: string | number | null; children?: ReactNode }) {
  const detail = typeof value === "string" ? value.trim() : Number.isFinite(value) ? value : null;
  return <div className="sys-row"><dt>{label}</dt><dd>{detail === "" || detail == null ? "Unavailable" : detail}{children}</dd></div>;
}
function diskProperty(value: string | number, labels: Record<number, string>, fallback: string) {
  if (typeof value === "string" && value.trim() && !Number.isFinite(Number(value))) return value;
  return value == null || value === "" ? fallback : labels[Number(value)] ?? fallback;
}
function Meter({ used, total }: { used: number; total: number }) {
  const percent = total > 0 ? Math.max(0, Math.min(100, used / total * 100)) : 0;
  return <div className="ram-meter" aria-hidden="true"><div className="ram-meter-fill" style={{ width: `${percent}%` }} /></div>;
}
export function SystemDetails({ info, section }: { info: SystemInfo; section: SystemSection }) {
  const hw = info.hardware;
  return <>
    <h3>{SYSTEM_SECTIONS.find(item => item.id === section)?.label}</h3>
    {section === "overview" && <dl className="sys-facts">
      <Row label="Processor" value={info.cpu} /><Row label="Graphics" value={info.gpu} />
      <Row label="Installed RAM" value={bytes(info.ramTotalBytes)} /><Row label="Available RAM" value={bytes(info.ramAvailableBytes)} />
      <Row label="Primary desktop" value={info.display} /><Row label="Active power plan" value={hw?.powerPlan} />
      <Row label="Windows" value={`${info.os} · ${info.osVersion}`} /><Row label="Uptime" value={formatUptime(info.uptimeSeconds)} />
    </dl>}
    {section === "processor" && <>
      <dl className="sys-facts"><Row label="Processor" value={info.cpu} /><Row label="Logical processors" value={info.cpuCores} />
        <Row label="Architecture" value={info.architecture} /><Row label="Reported nominal clock" value={info.cpuMhz ? `${info.cpuMhz} MHz` : null} />
        <Row label="Motherboard" value={hw?.board?.map(board => `${board.Manufacturer} ${board.Product}`).join(" · ")} />
        <Row label="BIOS version" value={hw?.bios?.map(bios => bios.SMBIOSBIOSVersion).join(" · ")} />
      </dl>
      {hw?.processors?.map((cpu, i) => <dl className="sys-facts system-device" key={i}>
        <Row label="Physical cores" value={cpu.NumberOfCores} /><Row label="L3 cache" value={cpu.L3CacheSize ? `${cpu.L3CacheSize / 1024} MB` : null} />
      </dl>)}
      <p className="system-note">Clock figures are hardware-reported specifications, not live boost clocks.</p>
    </>}
    {section === "memory" && <>
      <dl className="sys-facts"><Row label="Installed" value={bytes(info.ramTotalBytes)} /><Row label="Available" value={bytes(info.ramAvailableBytes)} />
        <Row label="In use" value={bytes(info.ramUsedBytes)}><Meter used={info.ramUsedBytes} total={info.ramTotalBytes} /></Row>
        <Row label="Populated memory slots" value={hw?.memory?.length} />
      </dl>
      {hw?.memory?.map((ram, i) => <div className="system-device" key={i}><h4>{ram.DeviceLocator || `Module ${i + 1}`}</h4>
        <dl className="sys-facts"><Row label="Capacity" value={bytes(ram.Capacity)} />
          <Row label="Configured transfer rate" value={ram.ConfiguredClockSpeed ? `${ram.ConfiguredClockSpeed} MT/s` : null} />
          <Row label="Manufacturer" value={ram.Manufacturer?.trim()} /><Row label="Part number" value={ram.PartNumber?.trim()} /></dl>
      </div>)}
    </>}
    {section === "graphics" && <>
      {hw?.graphics?.length ? hw.graphics.map((gpu, i) => <div className="system-device" key={i}><h4>{gpu.Name}</h4>
        <dl className="sys-facts"><Row label="Driver version" value={gpu.DriverVersion} /><Row label="GPU" value={gpu.VideoProcessor} /></dl>
      </div>) : <dl className="sys-facts"><Row label="Graphics adapter" value={info.gpu} /></dl>}
      <p className="system-note">GPU temperature, load and VRAM usage are not reported by this hardware snapshot.</p>
    </>}
    {section === "displays" && <>
      <dl className="sys-facts"><Row label="Primary desktop" value={info.display} /><Row label="Connected displays" value={info.monitors} /></dl>
      {hw?.graphics?.filter(gpu => gpu.CurrentHorizontalResolution > 0).map((gpu, i) => <div className="system-device" key={i}><h4>{gpu.Name}</h4>
        <dl className="sys-facts"><Row label="Adapter-reported resolution" value={`${gpu.CurrentHorizontalResolution} × ${gpu.CurrentVerticalResolution}`} />
          <Row label="Adapter-reported refresh rate" value={gpu.CurrentRefreshRate > 1 ? `${gpu.CurrentRefreshRate} Hz` : null} /></dl>
      </div>)}
      <p className="system-note">Windows adapter reports may not describe every monitor in a multi-display setup. HDR and variable refresh rate are not inferred.</p>
    </>}
    {section === "storage" && <>
      {(info.disks ?? []).map(disk => <div className="system-device" key={disk.mount}><h4>{disk.mount || disk.name}</h4>
        <dl className="sys-facts"><Row label="Free space" value={bytes(disk.availableBytes)} /><Row label="Capacity" value={bytes(disk.totalBytes)} /></dl>
        <Meter used={disk.totalBytes - disk.availableBytes} total={disk.totalBytes} />
      </div>)}
      {hw?.storage?.map((disk, i) => <div className="system-device" key={i}><h4>{disk.FriendlyName}</h4><dl className="sys-facts">
        <Row label="Media" value={diskProperty(disk.MediaType, { 3: "HDD", 4: "SSD" }, "Unspecified")} />
        <Row label="Bus" value={diskProperty(disk.BusType, { 1: "SCSI", 7: "USB", 8: "RAID", 11: "SATA", 17: "NVMe" }, "Other / unspecified")} />
        <Row label="Windows health status" value={diskProperty(disk.HealthStatus, { 0: "Healthy", 1: "Warning", 2: "Unhealthy" }, "Unknown")} />
        <Row label="Physical capacity" value={bytes(disk.Size)} /></dl></div>)}
    </>}
    {section === "windows" && <dl className="sys-facts">
      <Row label="Operating system" value={info.os} /><Row label="Version" value={info.osVersion} />
      <Row label="Build" value={info.kernel} /><Row label="Architecture" value={info.architecture} />
      <Row label="PC name" value={info.hostname} /><Row label="Uptime" value={formatUptime(info.uptimeSeconds)} />
      <Row label="Active power plan" value={hw?.powerPlan} />
    </dl>}
    {section !== "files" && <p className="system-note">Memory usage and free space refresh automatically. Hardware, drivers and power plan are captured once per launcher session; unsupported values show as unavailable.</p>}
  </>;
}
export function SystemPage({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [section, setSection] = useState<SystemSection>("overview");
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!open) return;
    let cancelled = false, timer: number | undefined;
    const load = async () => {
      try { const next = await invoke<SystemInfo>("system_info"); if (!cancelled) { setInfo(next); setError(null); } }
      catch (e) { if (!cancelled) setError(String(e)); }
      finally { if (!cancelled) timer = window.setTimeout(load, 2500); }
    };
    void load();
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } };
    document.addEventListener("keydown", key);
    return () => { cancelled = true; clearTimeout(timer); document.removeEventListener("keydown", key); };
  }, [open, onClose]);
  if (!open) return null;
  return <div className="settings-backdrop" onClick={onClose}>
    <div className="settings-panel settings-panel-wide sys-panel" onClick={event => event.stopPropagation()} role="dialog" aria-modal="true" aria-label="System">
      <div className="tools-heading"><h2>System</h2><button type="button" className="btn" onClick={onClose}>Close</button></div>
      <div className="settings-layout system-layout">
        <nav className="settings-nav" aria-label="System sections">{SYSTEM_SECTIONS.map(item =>
          <button type="button" key={item.id} className={`settings-nav-btn${section === item.id ? " active" : ""}`} aria-current={section === item.id ? "page" : undefined} onClick={() => setSection(item.id)}>{item.label}</button>)}</nav>
        <div className="settings-body" key={section}>
          {error && <p className="system-error" role="status">{error}</p>}
          {section === "files" ? <><h3>Files</h3><p className="system-note">Browse your game drives and folders in Windows File Explorer.</p>
            <button type="button" className="btn" onClick={() => void invoke("open_explorer", { path: null }).catch(e => setError(String(e)))}>Open File Explorer</button></>
            : info ? <SystemDetails info={info} section={section} /> : <p className="chart-empty">Reading hardware details…</p>}
        </div>
      </div>
    </div>
  </div>;
}
