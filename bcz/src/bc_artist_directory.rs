use std::{future::Future, pin::Pin, task::{Context, Poll}, sync::{Arc, RwLock}, collections::VecDeque, time::Duration};

use reqwest::{Client, Response, Error};
use scraper::{Html, Selector};
use tokio::{task::JoinHandle, time::{sleep, Sleep}};
use tokio_stream::Stream;
use tracing::{debug, info, warn, error};

use crate::{bc::BcScraper, ScraperState};

#[derive(Clone, Debug)]
pub struct ArtistUrl {
    pub name: String,
    pub url: String,
}

impl ArtistUrl {
    #[tracing::instrument(skip(r), fields(url = %r.url(), status = %r.status()))]
    async fn parse_from_response(r: Response) -> VecDeque<ArtistUrl> {
        let mut res = VecDeque::new();

        debug!("Will begin parsing the response and collect all artist urls.");
        let page_text = r.text().await.unwrap();
        let document = Html::parse_document(&page_text);
        let artist_selector = Selector::parse("li.item").unwrap();
        let url_selector = Selector::parse("a").unwrap();
        let artist_name_selector = Selector::parse("div.itemtext").unwrap();

        for element in document.select(&artist_selector) {
            let url = element.select(&url_selector).next().unwrap();
            let artist_url = url.value().attr("href").unwrap().to_owned();
            let name = url.select(&artist_name_selector).next().unwrap();
            let artist_name = name.text().next().unwrap().to_owned();

            res.push_back(ArtistUrl {
                name: artist_name,
                url: artist_url,
            });
        }

        info!(artist_urls_parsed = res.len(), "Parsed {} artist urls successfully.", res.len());

        res
    }
}

#[derive(Debug)]
enum ArtistsPollState {
    HasArtistsFetched,
    WaitingForPageFetch,
    WaitingForPageParse,
    WaitingForSleep,
}

pub struct Artists {
    poll_state: ArtistsPollState,
    client: Client,
    state: Arc<RwLock<ScraperState>>,
    fetched_artists: VecDeque<ArtistUrl>,
    artists_per_page: usize,
    current_fetch_task: Option<JoinHandle<Result<Response, Error>>>,
    current_sleep_task: Option<Pin<Box<Sleep>>>,
    current_page_parse_future: Option<Pin<Box<dyn Future<Output = VecDeque<ArtistUrl>>>>>,
    current_fetched_page_number: usize,
}

impl Artists {
    pub fn from(scraper: &BcScraper) -> Self {
        Self {
            poll_state: ArtistsPollState::HasArtistsFetched,
            client: scraper.client.clone(),
            state: scraper.state.clone(),
            fetched_artists: VecDeque::new(),
            artists_per_page: 0,
            current_fetch_task: None,
            current_sleep_task: None,
            current_page_parse_future: None,
            current_fetched_page_number: 0,
        }
    }

    fn next_artist_number(&mut self) -> usize {
        self.state.read().unwrap().next_artist_number
    }

    #[tracing::instrument(skip(self))]
    fn next_artist_page(&mut self) -> usize {
        if self.artists_per_page > 0 {
            let current_next_artist_number = self.next_artist_number();
            let remaining_artists_in_queue = self.fetched_artists.len();

            let next_artist_number_to_fetch = current_next_artist_number + remaining_artists_in_queue;

            // Adding 1 because we need to start counting from 0.
            let next_page = 1 + (next_artist_number_to_fetch / self.artists_per_page);

            debug!(self.artists_per_page, current_next_artist_number, remaining_artists_in_queue, next_artist_number_to_fetch, next_page, "Next artist page is {}.", next_page);

            next_page
        } else {
            1
        }
    }

    #[tracing::instrument(skip(self), fields(self.poll_state))]
    fn trigger_fetch_next_artist_page(&mut self) {
        if self.current_fetch_task.is_some() {
            debug!(current_fetch_task_exists = true, "Got called to trigger a new page fetch, but a fetch task already exists.");
            return;
        }

        info!("Triggering a new page fetch for artists.");

        let client = self.client.clone();
        let page = self.next_artist_page();

        info!(self.artists_per_page, page, "Going to fetch page {} for artists.", page);

        self.current_fetched_page_number = page;

        let fetch_task = tokio::spawn(async move {
            client.get("https://bandcamp.com/artist_index")
            .query(&[("sort_asc", "1"), ("page", &page.to_string())])
            .send()
            .await
        });

        self.current_fetch_task = Some(fetch_task);
    }
}

// TODO: figure out what to do when we finish going through all pages and get a page with empty results.
impl Stream for Artists {
    type Item = ArtistUrl;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.poll_state {
                ArtistsPollState::HasArtistsFetched => {
                    debug!(self.poll_state = ?self.poll_state, "Has artists fetched.");
                    if self.fetched_artists.len() <= 5 {
                        self.trigger_fetch_next_artist_page();
                    }

                    if !self.fetched_artists.is_empty() {
                        let artist_url_to_return = self.fetched_artists.pop_front();

                        let next_artist_number = {
                            // Scoped to allow the write lock to be dropped.
                            let mut state = self.state.write().unwrap();
                            state.next_artist_number += 1;
                            // Important: we'll also add the artist to the state to make sure we capture the fact we just returned it to the user.
                            state.new_artist_from_url(artist_url_to_return.clone().unwrap());
                            state.next_artist_number
                        };
                        debug!(next_artist_number, fetched_artists_len = self.fetched_artists.len(), "Still have fetched artists, returning the front one.");
                        return Poll::Ready(artist_url_to_return);
                    } else {
                        debug!(fetched_artists_len = self.fetched_artists.len(), "Ran out of fetched artists, will wait for page fetch.");
                        self.poll_state = ArtistsPollState::WaitingForPageFetch;
                    }
                }
                ArtistsPollState::WaitingForSleep => {
                    debug!(self.poll_state = ?self.poll_state, "Currently waiting for sleep, will poll the future now.");
                    match self.current_sleep_task.as_mut().unwrap().as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(_) => {
                            debug!(self.poll_state = ?self.poll_state, "Sleep has finished! Will request a new page fetch now.");
                            self.current_sleep_task = None;
                            self.trigger_fetch_next_artist_page();
                            self.poll_state = ArtistsPollState::WaitingForPageFetch;
                        }
                    }
                }
                ArtistsPollState::WaitingForPageFetch => {
                    debug!(self.poll_state = ?self.poll_state, "Currently waiting for page fetch, will poll the future now.");
                    match Pin::new(&mut self.current_fetch_task).as_pin_mut().unwrap().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(r) => {
                            debug!(self.poll_state = ?self.poll_state, "Page fetch has finished!");
                            self.current_fetch_task = None;

                            match r {
                                Err(e) => {
                                    panic!("Got a panic while fetching a page of artists! Error: {}", e);
                                }
                                Ok(Err(e)) => {
                                    error!(self.poll_state = ?self.poll_state, error = %e, "We got an error while fetching a page of artists, will try again in 100ms! Error: {}", e);

                                    self.current_sleep_task = Some(Box::pin(sleep(Duration::from_millis(100))));
                                    self.poll_state = ArtistsPollState::WaitingForSleep;
                                }
                                Ok(Ok(r)) => {
                                    debug!(self.poll_state = ?self.poll_state, "Got a page during fetch. Will now parse it.");
                                    self.current_page_parse_future = Some(Box::pin(ArtistUrl::parse_from_response(r)));
                                    self.poll_state = ArtistsPollState::WaitingForPageParse;
                                }
                            }
                        }
                    }
                }
                ArtistsPollState::WaitingForPageParse => {
                    debug!(self.poll_state = ?self.poll_state, "Currently waiting for page parsing, will poll the future now.");
                    match self.current_page_parse_future.as_mut().unwrap().as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(v) => {
                            debug!(self.poll_state = ?self.poll_state, artists_parsed = v.len(), "Page parsing has finished!");

                            if self.artists_per_page == 0 {
                                self.artists_per_page = v.len();
                                debug!(self.artists_per_page, "We didn't know how many artists per page we'd get, so we're updating this value now.");
                            }

                            // If this was the first page that we fetched just to populate the number of artists per page, and the artist we're looking for isn't in this page, just request a new one.
                            if self.next_artist_page() != self.current_fetched_page_number {
                                debug!(next_artist_number = self.next_artist_number(), self.current_fetched_page_number, "We fetched a page that didn't have the artist we're looking for, so we'll fetch a different page now.");
                                self.poll_state = ArtistsPollState::WaitingForPageFetch;
                                self.trigger_fetch_next_artist_page();
                            } else {
                                // We'll only get here once we ran out of artists in our current queue, so we can just exchange the queues.
                                self.fetched_artists = v;
                                self.poll_state = ArtistsPollState::HasArtistsFetched;
                            }
                        }
                    }
                }
            }
        }
    }
}