mod amazon;
mod battlenet;
mod ea;
mod epic;
mod gog;
mod humble;
mod itch;
mod manual;
pub mod riot;
mod roblox;
pub mod rockstar;
mod steam;
mod ubisoft;
mod wargaming;
mod xbox;

use crate::models::{DiscoveredGame, ScanStoreProgress};
use anyhow::Result;

pub fn scan_all(mut on_progress: impl FnMut(ScanStoreProgress)) -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();
    let scanners: &[(&str, fn() -> Result<Vec<DiscoveredGame>>)] = &[
        ("Steam", steam::scan),
        ("Epic", epic::scan),
        ("GOG", gog::scan),
        ("Battle.net", battlenet::scan),
        ("Ubisoft", ubisoft::scan),
        ("Xbox", xbox::scan),
        ("EA", ea::scan),
        ("Roblox", roblox::scan),
        ("Wargaming", wargaming::scan),
        ("Riot", riot::scan),
        ("Rockstar", rockstar::scan),
        ("Amazon", amazon::scan),
        ("itch.io", itch::scan),
        ("Humble", humble::scan),
    ];

    for (name, scan) in scanners {
        on_progress(ScanStoreProgress {
            store: (*name).to_string(),
            status: "scanning".into(),
            count: 0,
            message: None,
        });
        match scan() {
            Ok(mut g) => {
                let count = g.len();
                games.append(&mut g);
                on_progress(ScanStoreProgress {
                    store: (*name).to_string(),
                    status: if count == 0 { "empty".into() } else { "done".into() },
                    count,
                    message: None,
                });
            }
            Err(e) => {
                on_progress(ScanStoreProgress {
                    store: (*name).to_string(),
                    status: "error".into(),
                    count: 0,
                    message: Some(e.to_string()),
                });
                eprintln!("{name} scan: {e:#}");
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    games.retain(|g| seen.insert(g.id.clone()));
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(games)
}

pub use manual::create_manual_from_path;
