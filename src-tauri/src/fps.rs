use serde::Serialize;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const STILL_ACTIVE: u32 = 259;
const INVALID_HANDLE_VALUE: isize = -1;

#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
    fn Process32FirstW(h_snapshot: isize, lppe: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(h_snapshot: isize, lppe: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> isize;
    fn GetExitCodeProcess(h_process: isize, lp_exit_code: *mut u32) -> i32;
    fn CloseHandle(h_object: isize) -> i32;
}

static MONITOR_STOP: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsTick {
    pub game_id: String,
    pub fps: f64,
    pub process_alive: bool,
}

/// Start real-time FPS monitoring for a launched game executable.
/// Uses Intel PresentMon (downloaded once into app data). Safe no-op if unavailable.
pub fn start_monitoring(app: AppHandle, game_id: String, launch_target: String) {
    stop_monitoring();

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut guard = MONITOR_STOP.lock().unwrap();
        *guard = Some(stop.clone());
    }

    thread::spawn(move || {
        let exe_name = Path::new(&launch_target)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&launch_target)
            .to_string();

        // Wait briefly for the process to appear (store launchers take a moment)
        let mut pid: Option<u32> = None;
        for _ in 0..40 {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            pid = find_pid_by_exe(&exe_name);
            if pid.is_some() {
                break;
            }
            // Steam/Epic may start a different child — also try without extension
            let stem = Path::new(&exe_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if !stem.is_empty() {
                pid = find_pid_by_stem(stem);
                if pid.is_some() {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(500));
        }

        let Some(pid) = pid else {
            let _ = app.emit(
                "fps-tick",
                FpsTick {
                    game_id: game_id.clone(),
                    fps: 0.0,
                    process_alive: false,
                },
            );
            return;
        };

        let presentmon = match ensure_presentmon() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("PresentMon unavailable ({e:#}); FPS live capture disabled");
                // Keep emitting process-alive heartbeat so UI knows session is running
                while !stop.load(Ordering::SeqCst) {
                    let alive = pid_alive(pid);
                    let _ = app.emit(
                        "fps-tick",
                        FpsTick {
                            game_id: game_id.clone(),
                            fps: 0.0,
                            process_alive: alive,
                        },
                    );
                    if !alive {
                        break;
                    }
                    thread::sleep(Duration::from_secs(2));
                }
                return;
            }
        };

        let mut child = match Command::new(&presentmon)
            .args([
                "--stop_existing_session",
                "--process_id",
                &pid.to_string(),
                "--output_stdout",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to start PresentMon: {e}");
                return;
            }
        };

        let stdout = child.stdout.take();
        let mut last_emit = std::time::Instant::now() - Duration::from_secs(2);
        let mut sample_buf: Vec<f64> = Vec::new();
        let mut ms_col: Option<usize> = None;

        if let Some(out) = stdout {
            let reader = BufReader::new(out);
            for line in reader.lines().flatten() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if ms_col.is_none() && line.to_lowercase().contains("msbetweenpresents") {
                    ms_col = line.split(',').position(|c| {
                        c.trim()
                            .trim_matches('"')
                            .eq_ignore_ascii_case("msBetweenPresents")
                    });
                    continue;
                }
                if let Some(fps) = parse_presentmon_fps(&line, ms_col) {
                    if fps > 0.0 && fps < 1000.0 {
                        sample_buf.push(fps);
                    }
                }
                if last_emit.elapsed() >= Duration::from_secs(1) {
                    let avg = if sample_buf.is_empty() {
                        0.0
                    } else {
                        sample_buf.iter().sum::<f64>() / sample_buf.len() as f64
                    };
                    sample_buf.clear();
                    let alive = pid_alive(pid);
                    let _ = app.emit(
                        "fps-tick",
                        FpsTick {
                            game_id: game_id.clone(),
                            fps: avg,
                            process_alive: alive,
                        },
                    );
                    last_emit = std::time::Instant::now();
                    if !alive {
                        break;
                    }
                }
            }
        }

        let _ = child.kill();
        let _ = app.emit(
            "fps-tick",
            FpsTick {
                game_id,
                fps: 0.0,
                process_alive: false,
            },
        );
    });
}

pub fn stop_monitoring() {
    if let Ok(mut guard) = MONITOR_STOP.lock() {
        if let Some(flag) = guard.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

fn parse_presentmon_fps(line: &str, ms_col: Option<usize>) -> Option<f64> {
    if line.starts_with("Application") || line.starts_with("\"Application") {
        return None;
    }
    let parts: Vec<&str> = line.split(',').collect();
    if let Some(idx) = ms_col {
        let t = parts.get(idx)?.trim().trim_matches('"');
        let ms = t.parse::<f64>().ok()?;
        if (0.2..500.0).contains(&ms) {
            return Some(1000.0 / ms);
        }
    }
    // Fallback: scan numeric fields for plausible frame times
    for (i, part) in parts.iter().enumerate() {
        if i < 4 {
            continue;
        }
        let t = part.trim().trim_matches('"');
        if let Ok(ms) = t.parse::<f64>() {
            if (0.5..100.0).contains(&ms) {
                return Some(1000.0 / ms);
            }
        }
    }
    None
}

fn find_pid_by_exe(exe_name: &str) -> Option<u32> {
    find_pid_by_name(exe_name)
}

fn find_pid_by_stem(stem: &str) -> Option<u32> {
    // Toolhelp reports the image name (often with .exe); accept stem or stem.exe.
    find_pid_by_name(stem).or_else(|| find_pid_by_name(&format!("{stem}.exe")))
}

fn find_pid_by_name(name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    for_each_process(|pid, exe| {
        if exe.eq_ignore_ascii_case(name) {
            Some(pid)
        } else {
            None
        }
    })
}

fn for_each_process<F>(mut f: F) -> Option<u32>
where
    F: FnMut(u32, &str) -> Option<u32>,
{
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == 0 || snap == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut entry = ProcessEntry32W {
            dw_size: std::mem::size_of::<ProcessEntry32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0; 260],
        };

        let mut found = None;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry
                    .sz_exe_file
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.sz_exe_file.len());
                let exe = OsString::from_wide(&entry.sz_exe_file[..len]);
                if let Some(exe) = exe.to_str() {
                    if let Some(pid) = f(entry.th32_process_id, exe) {
                        found = Some(pid);
                        break;
                    }
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        found
    }
}

fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 || handle == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE
    }
}

fn tools_dir() -> PathBuf {
    crate::db::app_data_dir().join("tools")
}

fn ensure_presentmon() -> anyhow::Result<PathBuf> {
    let dir = tools_dir();
    fs_create(&dir)?;
    let dest = dir.join("PresentMon.exe");
    if dest.exists() && dest.metadata().map(|m| m.len() > 100_000).unwrap_or(false) {
        return Ok(dest);
    }

    // Official PresentMon releases (x64). URL may change with versions; try a few.
    let urls = [
        "https://github.com/GameTechDev/PresentMon/releases/download/v2.3.1/PresentMon-2.3.1-x64.exe",
        "https://github.com/GameTechDev/PresentMon/releases/download/v2.3.0/PresentMon-2.3.0-x64.exe",
    ];
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("IntelGenGameLauncher/0.2")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;

    let mut last_err = None;
    for url in urls {
        match client.get(url).send() {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes()?;
                if bytes.len() < 50_000 {
                    last_err = Some(anyhow::anyhow!("download too small"));
                    continue;
                }
                std::fs::write(&dest, &bytes)?;
                return Ok(dest);
            }
            Ok(resp) => {
                last_err = Some(anyhow::anyhow!("HTTP {}", resp.status()));
            }
            Err(e) => last_err = Some(e.into()),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("PresentMon download failed")))
}

fn fs_create(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    Ok(())
}
