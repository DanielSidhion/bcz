use std::{collections::HashMap, sync::{Arc, RwLock}};

use bc_artist_directory::ArtistUrl;
use bc_artist_page::ArtistDiscography;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};

pub mod bc;
mod bc_artist_directory;
mod bc_artist_page;

static BANDCAMP_DISCOGRAPHY_PATH: &'static str = "/music";

pub(crate) type RuntimeScraperState = Arc<RwLock<ScraperState>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct ArtistInfo {
    pub name: String,
    pub url: String,
    pub discography: ArtistDiscography,
    pub last_scrape_completed_on: DateTime<Utc>,
}

impl From<ArtistUrl> for ArtistInfo {
    fn from(artist_url: ArtistUrl) -> Self {
        ArtistInfo {
            name: artist_url.name,
            url: artist_url.url,
            discography: Default::default(),
            last_scrape_completed_on: Utc.timestamp_millis(0),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScraperState {
    pub artists: HashMap<String, ArtistInfo>,
    pub next_artist_number: usize,
}

impl ScraperState {
    pub fn new() -> Self {
        Self {
            artists: HashMap::new(),
            next_artist_number: 0,
        }
    }

    pub fn new_artist_from_url(&mut self, artist_url: ArtistUrl) {
        assert!(!self.artists.contains_key(&artist_url.name));
        self.artists.insert(artist_url.name.clone(), artist_url.into());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
