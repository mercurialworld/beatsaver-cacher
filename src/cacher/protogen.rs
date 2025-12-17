// PROTObuf GENerator. get it?

use beatsaver_api::models::map::{Map, MapDifficulty, MapVersion};

use crate::{
    cacher::get_map_mods,
    mapdata::{Difficulty, Ranked, RankedValue, Votes},
};

/// Converts the BeatSaver ranked values to a DumbRequestManager-readable format.
pub(crate) fn generate_protobuf_ranked_values(diff: &MapDifficulty) -> Ranked {
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

/// Converts mods needed by a map to a DumbRequestManager-readable format.
pub(crate) fn generate_protobuf_map_mods(map_version: &MapVersion) -> u32 {
    let map_mods = get_map_mods(map_version);

    (map_mods.cinema as u32)
        + ((map_mods.mapping_extensions as u32) << 1)
        + ((map_mods.chroma as u32) << 2)
        + ((map_mods.noodle_extensions as u32) << 3)
        + ((map_mods.vivify as u32) << 4)
}

/// Converts mods needed by a map difficulty to a DumbRequestManager-readable format.
pub(crate) fn generate_protobuf_diff_mods(diff: &MapDifficulty) -> u32 {
    (diff.cinema as u32)
        + ((diff.me as u32) << 1)
        + ((diff.chroma as u32) << 2)
        + ((diff.ne as u32) << 3)
        + ((diff.vivify as u32) << 4)
}

/// Converts each difficulty in a map on BeatSaver to a DumbRequestManager-readable format.
pub(crate) fn generate_protobuf_diffs(map_version: &MapVersion) -> Vec<Difficulty> {
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

/// Converts the curator field on BeatSaver to a DumbRequestManager-readable format, if it exists.
pub(crate) fn generate_protobuf_curator(map: &Map) -> Option<String> {
    if map.curator.is_some() {
        return Some(map.curator.as_ref().unwrap().name.clone());
    }

    None
}

/// Converts BeatSaver map upvotes/downvotes to a DumbRequestManager-readable format.
pub(crate) fn generate_protobuf_votes(up: i32, down: i32) -> Votes {
    Votes {
        up: u32::try_from(up).unwrap_or(0),
        down: u32::try_from(down).unwrap_or(0),
    }
}
