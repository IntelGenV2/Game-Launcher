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

use crate::models::DiscoveredGame;
use anyhow::Result;

pub fn scan_all() -> Result<Vec<DiscoveredGame>> {
    let mut games = Vec::new();

    for (name, result) in [
        ("Steam", steam::scan()),
        ("Epic", epic::scan()),
        ("GOG", gog::scan()),
        ("Battle.net", battlenet::scan()),
        ("Ubisoft", ubisoft::scan()),
        ("Xbox", xbox::scan()),
        ("EA", ea::scan()),
        ("Roblox", roblox::scan()),
        ("Wargaming", wargaming::scan()),
        ("Riot", riot::scan()),
        ("Rockstar", rockstar::scan()),
        ("Amazon", amazon::scan()),
        ("itch.io", itch::scan()),
        ("Humble", humble::scan()),
    ] {
        match result {
            Ok(mut g) => games.append(&mut g),
            Err(e) => eprintln!("{name} scan: {e:#}"),
        }
    }

    let mut seen = std::collections::HashSet::new();
    games.retain(|g| seen.insert(g.id.clone()));
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(games)
}

pub use manual::create_manual_from_path;
