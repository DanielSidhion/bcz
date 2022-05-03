use std::{error::Error, sync::{Arc, RwLock}};

use reqwest::{Url, Client};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::{bc_artist_directory::ArtistUrl, bc::BcScraper, ScraperState, RuntimeScraperState};

#[derive(Clone, Debug)]
pub struct ReleaseUrl {
    pub name: String,
    pub url: String,
}

pub struct Releases {
    fetched_releases: Vec<ReleaseUrl>,
}

impl Releases {
    pub async fn for_artist(artist_url: &ArtistUrl, scraper: &BcScraper) -> Result<Self, Box<dyn Error>> {
        let client = scraper.client.clone();

        Ok(Self {
            fetched_releases: parse_artist_discography_page(client, artist_url).await?,
        })
    }
}

impl Iterator for Releases {
    type Item = ReleaseUrl;

    fn next(&mut self) -> Option<Self::Item> {
        self.fetched_releases.pop()
    }
}

async fn parse_artist_discography_page(client: Client, artist_url: &ArtistUrl) -> Result<Vec<ReleaseUrl>, Box<dyn Error>> {
    let mut discography_url = Url::parse(&artist_url.url)?;
    discography_url.set_path(crate::BANDCAMP_DISCOGRAPHY_PATH);

    let discography_page = client.get(discography_url).send().await?.text().await.unwrap();
    let document = Html::parse_document(&discography_page);

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