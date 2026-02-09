use crate::{CinemaScraper, Film};
use reqwest::{header, Client};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Scraper for Circolo Cinematografico Enrico Pizzuti (Cinema Turroni Oderzo)
/// Example page: https://www.enricopizzuti.it/
pub struct EnricoPizzutiScraper {
    url: String,
}

impl EnricoPizzutiScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for EnricoPizzutiScraper {
    async fn fetch_films(
        &self,
        client: &Client,
    ) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(&self.url)
            .header(
                header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .error_for_status()?;

        let body = resp.text().await?;
        let document = Html::parse_document(&body);

        // Select all links that point to individual film pages
        let link_selector = Selector::parse("a[href*=\"/film/\"]")?;

        let mut seen_urls = HashSet::new();
        let mut films = Vec::new();

        for link in document.select(&link_selector) {
            let href = link.value().attr("href").unwrap_or("");
            if href.is_empty() {
                continue;
            }

            let full_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("https://www.enricopizzuti.it{}", href)
            };

            if seen_urls.contains(&full_url) {
                continue;
            }

            // Collect visible text inside the link
            let parts: Vec<String> = link
                .text()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();

            if parts.is_empty() {
                continue;
            }

            let candidate_title = &parts[0];

            // Skip generic "Vai al film" links; we want the card links
            if candidate_title.eq_ignore_ascii_case("vai al film") {
                continue;
            }

            seen_urls.insert(full_url.clone());

            let release_date = parts.get(1).cloned();
            let synopsis = parts.get(2).cloned();

            films.push(Film {
                title: candidate_title.clone(),
                url: full_url,
                poster_url: None,
                cast: None,
                release_date,
                running_time: None,
                synopsis,
                showtimes: None,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "enrico_pizzuti.xml".to_string()
    }
}

