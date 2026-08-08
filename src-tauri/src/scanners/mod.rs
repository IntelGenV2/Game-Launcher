mod battlenet;
mod ea;
mod epic;
mod gog;
mod manual;
mod roblox;
mod steam;
mod ubisoft;
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
