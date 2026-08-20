use crate::covers;
use crate::models::Game;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::io::Read;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMetadata {
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_year: Option<i64>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub steam_app_id: Option<String>,
}

pub fn fetch_for_game(game: &Game, _sgdb_key: Option<&str>) -> Result<GameMetadata> {
    let mut meta = GameMetadata::default();
    let steam_id = covers::steam_app_id_for(game);
    if let Some(id) = steam_id.clone() {
        meta.steam_app_id = Some(id.clone());
        if let Ok(Some(from_steam)) = steam_appdetails(&id) {
            merge(&mut meta, from_steam);
        }
    }

    if meta.description.is_none()
        || meta.developer.is_none()
        || meta.release_year.is_none()
        || meta.genres.is_empty()
    {
        let generic = game.name.trim().eq_ignore_ascii_case("roblox");
        if !generic {
            if let Ok(Some(wiki)) = wikipedia_metadata(&game.name) {
                merge(&mut meta, wiki);
            }
        }
    }

    Ok(meta)
}

fn merge(into: &mut GameMetadata, from: GameMetadata) {
    if into.developer.is_none() {
        into.developer = from.developer;
    }
    if into.publisher.is_none() {
        into.publisher = from.publisher;
    }
    if into.release_year.is_none() {
        into.release_year = from.release_year;
    }
    if into.description.is_none() {
        into.description = from.description;
    }
    if into.genres.is_empty() {
        into.genres = from.genres;
    }
    if into.steam_app_id.is_none() {
        into.steam_app_id = from.steam_app_id;
    }
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("IntelGenGameLauncher/0.2 (metadata)")
        .build()?)
}

fn steam_appdetails(app_id: &str) -> Result<Option<GameMetadata>> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={app_id}");
    let resp = client()?.get(&url).send()?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let json: serde_json::Value = resp.json()?;
    let data = json.get(app_id).and_then(|v| v.get("data"));
    let Some(data) = data else {
        return Ok(None);
    };

    let join_arr = |key: &str| -> Option<String> {
        data.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|s| !s.is_empty())
    };

    let genres = data
        .get("genres")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.get("description").and_then(|d| d.as_str()))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let year = data
        .pointer("/release_date/date")
        .and_then(|v| v.as_str())
        .and_then(parse_year);

    let description = data
        .get("short_description")
        .and_then(|v| v.as_str())
        .map(strip_html)
        .filter(|s| !s.is_empty());

    Ok(Some(GameMetadata {
        developer: join_arr("developers"),
        publisher: join_arr("publishers"),
        release_year: year,
        description,
        genres,
        steam_app_id: Some(app_id.to_string()),
        ..Default::default()
    }))
}

fn wikipedia_metadata(name: &str) -> Result<Option<GameMetadata>> {
    let client = client()?;
    let queries = [
        format!("{name} (video game)"),
        format!("{name} video game"),
        name.to_string(),
    ];
    for q in queries {
        let search_url = format!(
            "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=5",
            urlencoding::encode(&q)
        );
        let search: serde_json::Value = client.get(&search_url).send()?.json()?;
        let titles: Vec<String> = search
            .pointer("/query/search")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("title").and_then(|x| x.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        for title in titles {
            if let Some(meta) = wikipedia_page(&client, &title)? {
                return Ok(Some(meta));
            }
        }
    }
    Ok(None)
}

fn wikipedia_page(
    client: &reqwest::blocking::Client,
    title: &str,
) -> Result<Option<GameMetadata>> {
    let encoded = urlencoding::encode(title);
    let summary_url =
        format!("https://en.wikipedia.org/api/rest_v1/page/summary/{encoded}");
    let summary: serde_json::Value = match client.get(&summary_url).send() {
        Ok(r) if r.status().is_success() => r.json()?,
        _ => return Ok(None),
    };
    let extract = summary
        .get("extract")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let qid = summary
        .get("wikibase_item")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut meta = GameMetadata {
        description: extract,
        ..Default::default()
    };

    if let Some(qid) = qid {
        if let Ok(wd) = wikidata_entity(client, &qid) {
            merge(&mut meta, wd);
        }
    }
    Ok(Some(meta))
}

fn wikidata_entity(
    client: &reqwest::blocking::Client,
    qid: &str,
) -> Result<GameMetadata> {
    let url = format!(
        "https://www.wikidata.org/w/api.php?action=wbgetentities&ids={qid}&props=claims|labels&languages=en&format=json"
    );
    let mut resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Ok(GameMetadata::default());
    }
    let mut buf = Vec::new();
    std::io::Read::take(&mut resp, 350_001).read_to_end(&mut buf)?;
    if buf.len() > 350_000 {
        return Ok(GameMetadata::default());
    }
    let json: serde_json::Value = serde_json::from_slice(&buf)?;
    let entity = json.pointer(&format!("/entities/{qid}")).cloned();
    let Some(entity) = entity else {
        return Ok(GameMetadata::default());
    };

    let label = |id: &str| -> Option<String> {
        json.pointer(&format!("/entities/{id}/labels/en/value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let claim_ids = |prop: &str| -> Vec<String> {
        entity
            .pointer(&format!("/claims/{prop}"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        c.pointer("/mainsnak/datavalue/value/id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut extra_ids = claim_ids("P178");
    extra_ids.extend(claim_ids("P123"));
    extra_ids.extend(claim_ids("P136"));
    extra_ids.sort();
    extra_ids.dedup();

    let mut labels: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !extra_ids.is_empty() {
        let ids = extra_ids.join("|");
        let url = format!(
            "https://www.wikidata.org/w/api.php?action=wbgetentities&ids={ids}&props=labels&languages=en&format=json"
        );
        if let Ok(resp) = client.get(&url).send() {
            if let Ok(body) = resp.json::<serde_json::Value>() {
                if let Some(ents) = body.get("entities").and_then(|v| v.as_object()) {
                    for (id, ent) in ents {
                        if let Some(name) = ent.pointer("/labels/en/value").and_then(|v| v.as_str())
                        {
                            labels.insert(id.clone(), name.to_string());
                        }
                    }
                }
            }
        }
    }

    let names_for = |prop: &str| -> Vec<String> {
        claim_ids(prop)
            .into_iter()
            .filter_map(|id| labels.get(&id).cloned().or_else(|| label(&id)))
            .collect()
    };

    let developers = names_for("P178");
    let publishers = names_for("P123");
    let genres = names_for("P136");

    let year = entity
        .pointer("/claims/P577/0/mainsnak/datavalue/value/time")
        .and_then(|v| v.as_str())
        .and_then(|t| {
            // +2011-11-18T00:00:00Z
            t.trim_start_matches('+')
                .get(0..4)
                .and_then(|y| y.parse().ok())
        });

    Ok(GameMetadata {
        developer: nonempty_join(&developers),
        publisher: nonempty_join(&publishers),
        release_year: year,
        genres,
        ..Default::default()
    })
}

fn nonempty_join(parts: &[String]) -> Option<String> {
    let s = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn parse_year(s: &str) -> Option<i64> {
    Regex::new(r"(19|20)\d{2}")
        .ok()?
        .find(s)
        .and_then(|m| m.as_str().parse().ok())
}

fn strip_html(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap_or_else(|_| Regex::new("$^").expect("empty"));
    re.replace_all(s, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
