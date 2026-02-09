use crate::{CinemaScraper, Film};
use reqwest::{header, Client};
use serde::Deserialize;

/// Scraper for The Space Cinema (uses JSON API)
pub struct SpaceCinemaScraper {
    cinema_id: u32,
    showing_date: String,
}

impl SpaceCinemaScraper {
    pub fn new(cinema_id: u32, showing_date: String) -> Self {
        Self {
            cinema_id,
            showing_date,
        }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for SpaceCinemaScraper {
    async fn warm_up(&self, client: &Client) -> Result<(), Box<dyn std::error::Error>> {
        // Warm-up request to get fresh cookies/tokens
        client
            .get("https://www.thespacecinema.it/")
            .header(
                header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let api_url = format!(
            "https://www.thespacecinema.it/api/microservice/showings/cinemas/{}/films",
            self.cinema_id
        );

        #[derive(Debug, Deserialize)]
        struct ApiResponse {
            result: Vec<ApiFilm>,
        }

        #[derive(Debug, Deserialize)]
        struct ApiFilm {
            filmTitle: String,
            filmUrl: String,
            posterImageSrc: String,
            cast: String,
            releaseDate: String,
            runningTime: i32,
            synopsisShort: String,
        }

        let resp = client
            .get(&api_url)
            .header(
                header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36",
            )
            .header(
                header::ACCEPT,
                "application/json,text/javascript,*/*;q=0.1",
            )
            .query(&[
                ("showingDate", self.showing_date.as_str()),
                ("minEmbargoLevel", "3"),
                ("includesSession", "true"),
                ("includeSessionAttributes", "true"),
            ])
            .send()
            .await?
            .error_for_status()?;

        let body = resp.text().await?;
        let parsed: ApiResponse = serde_json::from_str(&body)?;

        Ok(parsed
            .result
            .into_iter()
            .map(|f| Film {
                title: f.filmTitle,
                url: f.filmUrl,
                poster_url: Some(f.posterImageSrc),
                cast: Some(f.cast),
                release_date: Some(f.releaseDate),
                running_time: Some(f.runningTime as u32),
                synopsis: Some(f.synopsisShort),
            })
            .collect())
    }

    fn rss_filename(&self) -> String {
        format!("space_cinema_{}.xml", self.cinema_id)
    }
}
