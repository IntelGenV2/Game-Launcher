use serde::Serialize;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub hostname: String,
    pub username: String,
    pub os: String,
    pub os_version: String,
    pub kernel: String,
    pub architecture: String,
    pub cpu: String,
    pub cpu_cores: u32,
    pub cpu_mhz: u32,
    pub ram_total_bytes: u64,
    pub ram_available_bytes: u64,
    pub ram_used_bytes: u64,
    pub gpu: String,
    pub display: String,
    pub monitors: u32,
    pub uptime_seconds: u64,
    pub disks: Vec<DiskInfo>,
    pub hardware: serde_json::Value,
}

#[repr(C)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
    fn GetPhysicallyInstalledSystemMemory(total_kilobytes: *mut u64) -> i32;
    fn GetTickCount64() -> u64;
    fn GetLogicalDriveStringsW(n_buffer_length: u32, lp_buffer: *mut u16) -> u32;
    fn GetDriveTypeW(lp_root_path_name: *const u16) -> u32;
    fn GetDiskFreeSpaceExW(
        lp_directory_name: *const u16,
        lp_free_bytes_available_to_caller: *mut u64,
        lp_total_number_of_bytes: *mut u64,
        lp_total_number_of_free_bytes: *mut u64,
    ) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(n_index: i32) -> i32;
}

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const SM_CMONITORS: i32 = 80;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_RAMDISK: u32 = 6;

pub fn collect() -> SystemInfo {
    let (total, avail, used) = memory();
    let (os, os_version) = os_info();
    let (width, height, monitors) = display();
    SystemInfo {
        hostname: std::env::var("COMPUTERNAME").unwrap_or_default(),
        username: std::env::var("USERNAME").unwrap_or_default(),
        os,
        os_version,
        kernel: kernel_string(),
        architecture: std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default(),
        cpu: registry_string(&[r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"], &["ProcessorNameString"])
            .unwrap_or_else(|| "Unknown processor".into()),
        cpu_cores: std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(0),
        cpu_mhz: cpu_mhz(),
        ram_total_bytes: total,
        ram_available_bytes: avail,
        ram_used_bytes: used,
        gpu: gpu_names().join(" · "),
        display: if width > 0 && height > 0 {
            format!("{width} × {height}")
        } else {
            String::new()
        },
        monitors,
        uptime_seconds: unsafe { GetTickCount64() / 1000 },
        disks: disks(),
        hardware: hardware_snapshot(),
    }
}

fn memory() -> (u64, u64, u64) {
    let (usable, avail) = unsafe {
        let mut status = MemoryStatusEx {
            dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
            dw_memory_load: 0,
            ull_total_phys: 0,
            ull_avail_phys: 0,
            ull_total_page_file: 0,
            ull_avail_page_file: 0,
            ull_total_virtual: 0,
            ull_avail_virtual: 0,
            ull_avail_extended_virtual: 0,
        };
        if GlobalMemoryStatusEx(&mut status) != 0 {
            (status.ull_total_phys, status.ull_avail_phys)
        } else {
            (0, 0)
        }
    };
    let installed = unsafe {
        let mut kb: u64 = 0;
        if GetPhysicallyInstalledSystemMemory(&mut kb) != 0 {
            kb.saturating_mul(1024)
        } else {
            0
        }
    };
    let used = usable.saturating_sub(avail);
    (installed.max(usable), avail, used)
}

fn display() -> (i32, i32, u32) {
    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        let monitors = GetSystemMetrics(SM_CMONITORS).max(0) as u32;
        (width, height, monitors)
    }
}

fn os_info() -> (String, String) {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(key) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") else {
        return ("Windows".into(), String::new());
    };
    let mut product: String = key.get_value("ProductName").unwrap_or_else(|_| "Windows".into());
    let display: String = key.get_value("DisplayVersion").unwrap_or_default();
    let build: String = key
        .get_value("CurrentBuildNumber")
        .or_else(|_| key.get_value("CurrentBuild"))
        .unwrap_or_default();
    if build.parse::<u32>().unwrap_or(0) >= 22000 {
        product = product.replace("Windows 10", "Windows 11");
    }
    let version = [display.trim(), build.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (product, version)
}

fn kernel_string() -> String {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(key) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") else {
        return String::new();
    };
    let build: String = key
        .get_value("CurrentBuildNumber")
        .or_else(|_| key.get_value("CurrentBuild"))
        .unwrap_or_default();
    let ubr: u32 = key.get_value("UBR").unwrap_or(0);
    if build.is_empty() {
        String::new()
    } else if ubr > 0 {
        format!("{build}.{ubr}")
    } else {
        build
    }
}

fn cpu_mhz() -> u32 {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
        .ok()
        .and_then(|key| key.get_value::<u32, _>("~MHz").ok())
        .unwrap_or(0)
}

fn disks() -> Vec<DiskInfo> {
    unsafe {
        let needed = GetLogicalDriveStringsW(0, std::ptr::null_mut());
        if needed == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u16; needed as usize];
        let n = GetLogicalDriveStringsW(needed, buf.as_mut_ptr());
        if n == 0 || n > needed {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut start = 0usize;
        for i in 0..n as usize {
            if buf[i] != 0 {
                continue;
            }
            if i > start {
                let mut root = buf[start..i].to_vec();
                if root.last() != Some(&0) {
                    root.push(0);
                }
                let dtype = GetDriveTypeW(root.as_ptr());
                if dtype == DRIVE_FIXED || dtype == DRIVE_REMOVABLE || dtype == DRIVE_RAMDISK {
                    let mut avail = 0u64;
                    let mut total = 0u64;
                    let mut free_total = 0u64;
                    if GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut total, &mut free_total) != 0 {
                        let label = String::from_utf16_lossy(&buf[start..i]);
                        let name = label.trim_end_matches('\\').to_string();
                        out.push(DiskInfo {
                            name,
                            mount: label,
                            total_bytes: total,
                            available_bytes: avail,
                        });
                    }
                }
            }
            start = i + 1;
        }
        out
    }
}

fn gpu_names() -> Vec<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(class) = hklm.open_subkey(
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}",
    ) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for key in class.enum_keys().filter_map(|k| k.ok()) {
        if key.len() != 4 || !key.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(sub) = class.open_subkey(&key) else {
            continue;
        };
        let name: String = sub
            .get_value("DriverDesc")
            .or_else(|_| sub.get_value("AdapterString"))
            .unwrap_or_default();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("microsoft basic") || lower.contains("remote desktop") {
            continue;
        }
        if !names.iter().any(|existing: &String| existing.eq_ignore_ascii_case(trimmed)) {
            names.push(trimmed.to_string());
        }
    }
    names
}

fn registry_string(paths: &[&str], values: &[&str]) -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for path in paths {
        let Ok(key) = hklm.open_subkey(path) else {
            continue;
        };
        for value in values {
            if let Ok(raw) = key.get_value::<String, _>(value) {
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}


// CIM queries run once off the UI thread. Live memory/drive figures remain cheap to refresh.
fn hardware_snapshot() -> serde_json::Value {
    static SNAPSHOT: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    SNAPSHOT.get_or_init(|| {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$activePlan = Get-CimInstance -Namespace root/cimv2/power -ClassName Win32_PowerPlan -Filter 'IsActive = TRUE' | Select-Object -First 1
@{
  processors = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed,L3CacheSize)
  memory = @(Get-CimInstance Win32_PhysicalMemory | Select-Object DeviceLocator,Capacity,ConfiguredClockSpeed,Speed,Manufacturer,PartNumber)
  graphics = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,VideoProcessor,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate)
  storage = @(Get-PhysicalDisk | Select-Object FriendlyName,MediaType,BusType,HealthStatus,Size)
  board = @(Get-CimInstance Win32_BaseBoard | Select-Object Manufacturer,Product)
  bios = @(Get-CimInstance Win32_BIOS | Select-Object SMBIOSBIOSVersion)
  powerPlan = [string]$activePlan.ElementName
} | ConvertTo-Json -Depth 4 -Compress
"#;
        let Ok(mut child) = Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x08000000).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                return serde_json::Value::Null;
            };
        // Drain stdout concurrently: even a large multi-drive report must not fill the pipe.
        let stdout = child.stdout.take();
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            if let Some(stdout) = stdout { let _ = stdout.take(1024 * 1024).read_to_end(&mut bytes); }
            bytes
        });
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if start.elapsed().as_secs() < 12 => std::thread::sleep(std::time::Duration::from_millis(50)),
                _ => { let _ = child.kill(); let _ = child.wait(); return serde_json::Value::Null; }
            }
        }
        reader.join().ok()
            .and_then(|output| serde_json::from_slice(&output).ok())
            .unwrap_or(serde_json::Value::Null)
    }).clone()
}
