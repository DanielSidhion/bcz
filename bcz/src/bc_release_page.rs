use std::error::Error;

use reqwest::Client;
use scraper::{Html, Selector};
use tracing::debug;

use crate::{bc_artist_page::ReleaseUrl, Release, Track};

async fn parse_release_page(client: Client, release_url: &ReleaseUrl) -> Result<Release, Box<dyn Error>> {
    let discography_page = client.get(&release_url.url).send().await?.text().await.unwrap();
    let document = Html::parse_document(&discography_page);

    let release_name_selector = Selector::parse("h2.trackTitle").unwrap();
    let artist_name_selector = Selector::parse("#name-section > h3:nth-child(2) > span:nth-child(1) > a:nth-child(1)").unwrap();

    let release_name = document.select(&release_name_selector).next().unwrap().text().next().unwrap().to_owned();
    let artist_name = document.select(&artist_name_selector).next().unwrap().text().next().unwrap().to_owned();

    let release_purchase_methods_selector = Selector::parse("li.buyItem").unwrap();
    let purchase_method_selector = Selector::parse(".buyItemPackageTitle").unwrap();

    let single_duration_selector = Selector::parse(".time_total").unwrap();

    let mut release: Release;

    for method in document.select(&release_purchase_methods_selector) {
        let method_type = method.select(&purchase_method_selector).next().unwrap().text().next().unwrap();

        match method_type {
            "Digital Track" => {
                // For sure a single.
                let duration = document.select(&single_duration_selector).next().unwrap().text().next().unwrap();

                release = Release::Single {
                    name: release_name.clone(),
                    tracks: vec![
                        Track {
                            index: 1,
                            name: release_name.clone(),
                            duration: duration.to_owned(),
                        },
                    ],
                };
            }
            "Digital Album" => {
                // TODO: check if single track in the album, and if so figure out whether to consider this a single or album. A single might also have more than one track, so wtf do I do.
                // https://support.tunecore.com/hc/en-us/articles/115006689928-What-is-the-difference-between-a-Single-an-EP-and-an-Album-
                // https://support.symdistro.com/hc/en-us/articles/215985603-What-is-the-difference-between-Single-EP-and-Album-
            }
            "CD" => {

            }
            "Cassette Tape" => {

            }
            t if t.contains("Vinyl") || t.contains("vinyl") => {

            }
            t => {
                debug!("Skipping unwanted purchase method \"{}\".", t);
            }
        }
    }

    Ok(release)
}