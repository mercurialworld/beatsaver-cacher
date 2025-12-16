pub mod protogen;

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
use flate2::{Compression, write::GzEncoder};
use log::{debug, error, info};
use prost::Message;
use std::io::prelude::*;
use tokio::time::sleep;

use crate::mapdata::{Difficulty, MapList, MapMetadata, Ranked, RankedValue, Votes};
use crate::cacher::protogen::{
    generate_protobuf_curator,
    generate_protobuf_diff_mods,
    generate_protobuf_diffs,
    generate_protobuf_map_mods,
    generate_protobuf_ranked_values, 
    generate_protobuf_votes
}

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
        info!("{} hasn't been published before, ignoring", map.id);
        return false;
    }

    // version of map hasn't been published
    if map.versions[0].state != MapState::Published {
        info!("Version of {} is not published, ignoring", map.id);
        return false;
    }

    // AI-generated (map or song)
    if map.declared_ai != AIDeclarationType::None {
        info!("{} has been declared as AI-generated, ignoring", map.id);
        return false;
    }

    if map.automapper {
        info!("{} is automapped, ignoring", map.id);
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

pub fn cache_map_data(map: &Map) -> Option<MapMetadata> {
    if !should_cache_map(map) {
        debug!("Not caching {:?}", map.id);
        return None;
    }

    // now we make the map data
    let cached_map = MapMetadata {
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
        curator_name: generate_protobuf_curator(map),
        votes: generate_protobuf_votes(map.stats.upvotes, map.stats.downvotes),
        difficulties: generate_protobuf_diffs(&map.versions[0]),
    };

    Some(cached_map)
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
                debug!("Obtained {} maps", data.docs.len());

                if data.docs.is_empty() {
                    info!("[Scraper] No maps left!");
                    caching = false;
                } else {
                    for map_data in data.docs {
                        let map_key = map_data.id.clone();

                        if let Some(cached_map) = cache_map_data(&map_data) {
                            map_list.map_metadata.insert(map_key.clone(), cached_map);
                            last_map = Some(map_data);
                        }
                    }

                    info!("[Scraper] Cached {} maps", map_list.map_metadata.len(),);

                    if let Some(ref map) = last_map {
                        debug!("Currently at {}", map.id);
                        current_time = map.uploaded;

                        debug!("current_time set to {}", current_time);
                    }

                    sleep(Duration::from_millis(100)).await;
                }
            }
            Err(err) => match err {
                ClientError::ReqwestError(reqwest_err) => {
                    error!(
                        "Status not 200 (is {:?}), waiting a bit",
                        reqwest_err.status()
                    );
                    error!("{:?}", reqwest_err);
                    sleep(Duration::from_millis(3000)).await;
                    continue;
                }
                ClientError::SerdeError(serde_err) => {
                    error!("ERROR: {}", serde_err);
                }
                _ => unreachable!(""),
            },
        }
    }

    map_list
}

// [TODO] better return type
// [TODO] validation on this
pub async fn write_cache(map_list: &MapList, path: &str) -> bool {
    let buf = Vec::new();

    let mut gz = GzEncoder::new(buf, Compression::default());
    let _ = gz.write_all(&map_list.encode_to_vec());

    let compressed = gz.finish().unwrap();

    match fs::write(path, compressed) {
        Ok(_) => {
            info!("Saved to {}", path);
        }
        Err(e) => {
            error!("{:?}", e);
            return false;
        }
    }

    true
}
