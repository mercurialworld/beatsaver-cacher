use std::{
    collections::HashMap,
    fs::{self},
    time::Duration,
};

use beatsaver_api::{
    builders::BeatSaverMapSearchBuilder,
    client::{BeatSaverClient, ClientError},
    models::{
        enums::{AIDeclarationType, MapState},
        map::{Map, MapDetail, MapDifficulty, MapVersion},
    },
};
use prost::Message;
use tokio::time::sleep;

use crate::mapdata::mapdata::{Difficulty, MapList, MapMetadata, Ranked, RankedValue, Votes};

#[derive(Default)]
struct MapMods {
    pub cinema: bool,
    pub mapping_extensions: bool,
    pub chroma: bool,
    pub noodle_extensions: bool,
    pub vivify: bool,
}

fn should_cache_map(map: &Map) -> bool {
    // not published yet
    if map.last_published_at.is_none() {
        println!("{} hasn't been published before, ignoring", map.id);
        return false;
    }

    // version of map hasn't been published
    if map.versions[0].state != MapState::Published {
        println!("Version of {} is not published, ignoring", map.id);
        return false;
    }

    // AI-generated (map or song)
    if map.declared_ai != AIDeclarationType::None {
        println!("{} has been declared as AI-generated, ignoring", map.id);
        return false;
    }

    if map.automapper {
        println!("{} is automapped, ignoring", map.id);
        return false;
    }

    true
}

fn get_map_mods(map_version: &MapVersion) -> MapMods {
    let mut mods = MapMods::default();

    // O(n) woohoo!
    for diff in &map_version.diffs {
        // surely there's a better way
        if diff.chroma {
            mods.chroma = true;
        }

        if diff.cinema {
            mods.cinema = true;
        }

        if diff.me {
            mods.mapping_extensions = true;
        }

        if diff.ne {
            mods.noodle_extensions = true;
        }

        if diff.vivify {
            mods.vivify = true;
        }
    }

    mods
}

fn generate_protobuf_ranked_values(diff: &MapDifficulty) -> Ranked {
    // autogen moment. i kinda don't want to deal with renaming
    Ranked {
        score_saber: RankedValue {
            is_ranked: diff.ss_stars.is_some(),
            stars: diff.ss_stars.unwrap_or(0.0) as f32,
        },
        beat_leader: RankedValue {
            is_ranked: diff.bl_stars.is_some(),
            stars: diff.bl_stars.unwrap_or(0.0) as f32,
        },
    }
}

fn generate_protobuf_map_mods(map_version: &MapVersion) -> u32 {
    let map_mods = get_map_mods(map_version);

    (map_mods.cinema as u32)
        + ((map_mods.mapping_extensions as u32) << 1)
        + ((map_mods.chroma as u32) << 2)
        + ((map_mods.noodle_extensions as u32) << 3)
        + ((map_mods.vivify as u32) << 4)
}

fn generate_protobuf_diff_mods(diff: &MapDifficulty) -> u32 {
    (diff.cinema as u32)
        + ((diff.me as u32) << 1)
        + ((diff.chroma as u32) << 2)
        + ((diff.ne as u32) << 3)
        + ((diff.vivify as u32) << 4)
}

fn generate_protobuf_diffs(map_version: &MapVersion) -> Vec<Difficulty> {
    let mut diffs: Vec<Difficulty> = Vec::new();

    for diff in &map_version.diffs {
        diffs.push(Difficulty {
            njs: diff.njs as f32,
            notes: u32::try_from(diff.notes).unwrap_or(0),
            characteristic_name: diff.characteristic.name().to_string(),
            difficulty_name: diff.difficulty.clone(),
            mods: generate_protobuf_diff_mods(diff),
            environment_name: diff.environment.as_ref().unwrap().name().to_string(),
            ranked: generate_protobuf_ranked_values(diff),
        });
    }

    diffs
}

fn generate_protobuf_votes(up: i32, down: i32) -> Votes {
    Votes {
        up: u32::try_from(up).unwrap_or(0),
        down: u32::try_from(down).unwrap_or(0),
    }
}

pub fn cache_map_data(map: &Map) -> Option<MapMetadata> {
    if !should_cache_map(map) {
        return None;
    }

    // now we make the map data
    let map = MapMetadata {
        key: u32::from_str_radix(&map.id, 16).unwrap(),
        hash: map.versions[0].hash.clone(),
        song_name: map.metadata.song_name.clone(),
        song_sub_name: map.metadata.song_sub_name.clone(),
        song_author_name: map.metadata.song_author_name.clone(),
        level_author_name: map.metadata.level_author_name.clone(),
        duration: u32::try_from(map.metadata.duration).ok().unwrap(),
        uploaded: u32::try_from(map.last_published_at?.timestamp())
            .ok()
            .unwrap(),
        last_updated: u32::try_from(map.updated_at?.timestamp()).ok().unwrap(),
        mods: generate_protobuf_map_mods(&map.versions[0]),
        curator_name: Some(map.curator.clone()?.name),
        votes: generate_protobuf_votes(map.stats.upvotes, map.stats.downvotes),
        difficulties: generate_protobuf_diffs(&map.versions[0]),
    };

    Some(map)
}

pub async fn init_cache(client: &BeatSaverClient) -> MapList {
    let mut caching = true;
    let mut current_time = chrono::Utc::now();
    let mut last_map: Option<MapDetail> = None;

    let mut map_list: MapList = MapList {
        map_metadata: HashMap::new(),
    };

    while caching {
        let params = BeatSaverMapSearchBuilder::new()
            .before(current_time)
            .page_size(100)
            .automapper(false)
            .build();

        let res = client.latest(&params).await;

        match res {
            Ok(data) => {
                if data.docs.is_empty() {
                    println!("[Scraper] No maps left!");
                    caching = false;
                } else {
                    for map_data in data.docs {
                        let map_key = map_data.id.clone();

                        if let Some(cached_map) = cache_map_data(&map_data) {
                            println!("On {map_key}");

                            map_list.map_metadata.insert(map_key.clone(), cached_map);
                            last_map = Some(map_data);
                        }
                    }

                    println!("[Scraper] Cached {} maps", map_list.map_metadata.len(),);

                    if let Some(ref map) = last_map {
                        println!("Currently at {}", map.id);
                        current_time = map.uploaded;
                    }

                    sleep(Duration::from_millis(100)).await;
                }
            }
            Err(err) => match err {
                ClientError::ReqwestError(reqwest_err) => {
                    println!(
                        "Status not 200 (is {:?}), waiting a bit",
                        reqwest_err.status()
                    );
                    println!("{:?}", reqwest_err);
                    sleep(Duration::from_millis(3000)).await;
                    continue;
                }
                ClientError::SerdeError(serde_err) => {
                    eprintln!("ERROR: {}", serde_err);
                }
                _ => unreachable!(""),
            },
        }
    }

    map_list
}

// [TODO] better return type
pub async fn write_cache(map_list: &MapList, path: &str) -> bool {
    let mut buf = Vec::new();
    match map_list.encode(&mut buf) {
        Ok(_) => {
            println!("Serialized to {} bytes", buf.len());
        }
        Err(e) => {
            println!("{:?}", e);
            return false;
        }
    }

    match fs::write(path, &buf) {
        Ok(_) => {
            println!("Saved to {}", path);
        }
        Err(e) => {
            println!("{:?}", e);
            return false;
        }
    }

    true
}
