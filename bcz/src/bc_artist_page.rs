use std::{error::Error, sync::{Arc, RwLock}};

use reqwest::{Url, Client};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::{bc_artist_directory::ArtistUrl, bc::BcScraper, ScraperState, RuntimeScraperState};

#[derive(Debug, Deserialize, Serialize)]
pub struct Track {
    pub index: usize,
    pub name: String,
    pub duration: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Release {
    pub name: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ArtistDiscography {
    albums: Vec<Release>,
    eps: Vec<Release>,
    singles: Vec<Release>,
}

#[derive(Clone, Debug)]
pub struct ReleaseUrl {
    pub name: String,
    pub url: String,
}

pub struct Releases {
    client: Client,
    state: Arc<RwLock<ScraperState>>,
    fetched_releases: Vec<ReleaseUrl>,
}

impl Releases {
    pub fn from(scraper: &BcScraper) -> Self {
        Self {
            client: scraper.client.clone(),
            state: scraper.state.clone(),
            fetched_releases: Vec::new(),
        }
    }
}

impl Iterator for Releases {
    type Item = ReleaseUrl;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO
    }
}

// What about artists with their own custom domain/page?

async fn get_artist_discography_page(artist_url: &ArtistUrl, client: &Client) -> Result<String, Box<dyn Error>> {
    let mut discography_url = Url::parse(&artist_url.url)?;
    discography_url.set_path(crate::BANDCAMP_DISCOGRAPHY_PATH);

    Ok(client.get(discography_url).send().await?.text().await.unwrap())
}

async fn parse_artist_discography_page(page_text: String, state: RuntimeScraperState) -> Result<Vec<ReleaseUrl>, Box<dyn Error>> {
    let document = Html::parse_document(&page_text);

    let album_selector = Selector::parse("li.music-grid-item").unwrap();
    let url_selector = Selector::parse("a").unwrap();
    let release_name_selector = Selector::parse("p").unwrap();

    let mut res = Vec::new();

    for element in document.select(&album_selector) {
        let url = element.select(&url_selector).next().unwrap();
        let release_url = url.value().attr("href").unwrap().to_owned();
        let name = url.select(&release_name_selector).next().unwrap();
        let release_name = name.text().next().unwrap().to_owned();

        res.push(ReleaseUrl {
            name: release_name,
            url: release_url,
        });
    }

    Ok(res)
}