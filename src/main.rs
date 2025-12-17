use beatsaver_api::client::BeatSaverClient;

use crate::cacher::{init_cache, write_cache};

mod cacher;

pub(crate) mod mapdata {
    include!(concat!(env!("OUT_DIR"), "\\cached_beat_saver_data.rs"));
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let beatsaver_api = BeatSaverClient::default();

    let maps = init_cache(&beatsaver_api).await;

    write_cache(&maps, "mapData.proto.gz").await;
}
