use std::{sync::{Arc, RwLock}, time::Duration, error::Error};

use reqwest::{Client, Url};

pub use crate::bc_artist_directory::Artists;
use crate::{ScraperState, bc_artist_page::{ArtistDiscography}, bc_artist_directory::ArtistUrl, RuntimeScraperState};

pub struct BcScraper {
    pub(crate) client: Client,
    pub(crate) state: RuntimeScraperState,
}

impl BcScraper {
    pub fn with_state(state: ScraperState) -> Self {
        // TODO: also set a rate limiter.
        let client = Client::builder()
            .user_agent("bcz/0.1")
            .gzip(true)
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_millis(700))
            .build()
            .unwrap();

        Self {
            client,
            state: Arc::new(RwLock::new(state)),
        }
    }

    pub fn artists(&self) -> Artists {
        Artists::from(self)
    }

    pub async fn parse_artist_discography(&self, artist_url: &ArtistUrl) -> Result<ArtistDiscography, Box<dyn Error>> {
        let page_text = get_artist_discography_page(artist_url, &self.client).await?;

        let albums = vec![];
        let eps = vec![];
        let singles = vec![];

        Ok(ArtistDiscography {
            albums,
            eps,
            singles,
        })
    }
}