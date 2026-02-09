use crate::{CinemaScraper, Film};
use reqwest::{header, Client};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Scraper for Cinema Edera (uses HTML scraping)
pub struct CinemaEderaScraper {
    url: String,
}

impl CinemaEderaScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for CinemaEderaScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
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

        // Find the timetable table
        let table_selector = Selector::parse("#timetable")?;
        let table = document
            .select(&table_selector)
            .next()
            .ok_or("Could not find timetable table")?;

        // Select all rows in the table
        let row_selector = Selector::parse("tbody tr")?;
        let link_selector = Selector::parse("a.category__item")?;
        let title_selector = Selector::parse("strong")?;

        // Use HashSet to deduplicate films by URL (same film can appear on multiple days)
        let mut seen_urls = HashSet::new();
        let mut films = Vec::new();

        // Iterate through each row (each row represents a day)
        for row in table.select(&row_selector) {
            // Find all film links in this row's second column
            for link in row.select(&link_selector) {
                let href = link.value().attr("href").unwrap_or("");
                let full_url = format!("https://www.cinemaedera.it{}", href);

                // Skip if we've already seen this film
                if seen_urls.contains(&full_url) {
                    continue;
                }

                // Extract title from <strong> tag
                let title = link
                    .select(&title_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| {
                        // Fallback: get text from the link itself, but skip the <em> part
                        let mut text_parts = Vec::new();
                        for text_node in link.text() {
                            let trimmed = text_node.trim();
                            if !trimmed.is_empty() && !trimmed.starts_with("Orari:") {
                                text_parts.push(trimmed);
                            }
                        }
                        text_parts.join(" ").trim().to_string()
                    });

                if !title.is_empty() && !href.is_empty() {
                    seen_urls.insert(full_url.clone());
                    films.push(Film {
                        title,
                        url: full_url,
                        poster_url: None,
                        cast: None,
                        release_date: None,
                        running_time: None,
                        synopsis: None,
                    });
                }
            }
        }

        Ok(films)
    }
}
