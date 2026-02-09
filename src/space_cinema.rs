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
        #[allow(non_snake_case)]
        struct ApiSession {
            startTime: String,
            endTime: String,
        }

        #[derive(Debug, Deserialize)]
        struct ShowingGroup {
            sessions: Option<Vec<ApiSession>>,
        }

        #[derive(Debug, Deserialize)]
        #[allow(non_snake_case)]
        struct ApiFilm {
            filmTitle: String,
            filmUrl: String,
            posterImageSrc: String,
            cast: String,
            releaseDate: String,
            runningTime: i32,
            synopsisShort: String,
            showingGroups: Option<Vec<ShowingGroup>>,
        }

        /// Extract "HH:MM" from ISO datetime like "2026-02-09T22:45:00"
        fn time_part(s: &str) -> String {
            s.split('T')
                .nth(1)
                .and_then(|t| t.get(..5))
                .unwrap_or(s)
                .to_string()
        }

        /// Format ISO date "2026-02-09" as "09 Febbraio 2026"
        fn format_date_italian(s: &str) -> String {
            const MONTHS: [&str; 12] = [
                "Gennaio", "Febbraio", "Marzo", "Aprile", "Maggio", "Giugno",
                "Luglio", "Agosto", "Settembre", "Ottobre", "Novembre", "Dicembre",
            ];
            let date_str = s.get(..10).unwrap_or("");
            let parts: Vec<&str> = date_str.split('-').collect();
            if parts.len() != 3 {
                return s.to_string();
            }
            let year = parts[0];
            let month: usize = parts[1].parse().unwrap_or(0);
            let day = parts[2];
            if month == 0 || month > 12 {
                return s.to_string();
            }
            format!("{} {} {}", day, MONTHS[month - 1], year)
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

        let films: Vec<Film> = parsed
            .result
            .into_iter()
            .map(|f| {
                let showtimes = f.showingGroups.map(|groups| {
                    groups
                        .into_iter()
                        .filter_map(|g| g.sessions)
                        .flatten()
                        .map(|s| {
                            let date = format_date_italian(&s.startTime);
                            format!("{} ore {} - {}", date, time_part(&s.startTime), time_part(&s.endTime))
                        })
                        .collect::<Vec<_>>()
                }).filter(|v: &Vec<String>| !v.is_empty());

                Film {
                    title: f.filmTitle,
                    url: f.filmUrl,
                    poster_url: Some(f.posterImageSrc),
                    cast: Some(f.cast),
                    release_date: Some(f.releaseDate),
                    running_time: Some(f.runningTime as u32),
                    synopsis: Some(f.synopsisShort),
                    showtimes,
                }
            })
            .collect();

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        format!("space_cinema_{}.xml", self.cinema_id)
    }
}
