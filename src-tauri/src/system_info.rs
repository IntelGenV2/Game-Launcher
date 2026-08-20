use serde::Serialize;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub os_version: String,
    pub cpu: String,
    pub cpu_cores: u32,
    pub ram_total_bytes: u64,
    pub ram_available_bytes: u64,
    pub ram_used_bytes: u64,
    pub gpu: String,
    pub display: String,
    pub monitors: u32,
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
}

#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(n_index: i32) -> i32;
}

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const SM_CMONITORS: i32 = 80;

pub fn collect() -> SystemInfo {
    let (total, avail, used) = memory();
    let (os, os_version) = os_info();
    let (width, height, monitors) = display();
    SystemInfo {
        hostname: std::env::var("COMPUTERNAME").unwrap_or_default(),
        os,
        os_version,
        cpu: registry_string(&[r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"], &["ProcessorNameString"])
            .unwrap_or_else(|| "Unknown processor".into()),
        cpu_cores: std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(0),
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
