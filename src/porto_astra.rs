use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Scraper for Cinema Porto Astra Padova (fetches individual film pages).
pub struct PortoAstraScraper {
    url: String,
}

impl PortoAstraScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for PortoAstraScraper {
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
        // Limit lifetime of Html to avoid crossing await boundaries
        let urls: HashSet<String> = {
            let listing = Html::parse_document(&body);

            // Collect unique film URLs from listing page
            let link_sel = Selector::parse("a[href*=\"/film/\"]")?;
            let mut urls = HashSet::new();

            for a in listing.select(&link_sel) {
                if let Some(href) = a.value().attr("href") {
                    let href = href.trim();
                    if href.is_empty() {
                        continue;
                    }
                    let full = if href.starts_with("http") {
                        href.to_string()
                    } else if href.starts_with('/') {
                        format!("https://portoastra.it{}", href)
                    } else {
                        format!("https://portoastra.it/{}", href)
                    };
                    urls.insert(full);
                }
            }

            urls
        };

        if urls.is_empty() {
            return Ok(Vec::new());
        }

        let mut films = Vec::new();

        // For each film page, extract title, poster, metadata, synopsis.
        for url in urls {
            let resp = match client
                .get(&url)
                .header(
                    header::USER_AGENT,
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/143.0.0.0 Safari/537.36",
                )
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };

            let resp = match resp.error_for_status() {
                Ok(r) => r,
                Err(_) => continue,
            };

            let body = match resp.text().await {
                Ok(b) => b,
                Err(_) => continue,
            };

            let doc = Html::parse_document(&body);

            // Title: try <h1>/<h2>/<h3>, then first strong/bold text
            let mut title = None;
            if let Ok(h_sel) = Selector::parse("h1, h2, h3")
                && let Some(h) = doc.select(&h_sel).next()
            {
                let t = h
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !t.is_empty() {
                    title = Some(t);
                }
            }
            if title.is_none()
                && let Ok(b_sel) = Selector::parse("b, strong")
            {
                for b in doc.select(&b_sel) {
                    let t = b
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !t.is_empty() && !t.contains("REGIA") && !t.contains("ATTORI") {
                        title = Some(t);
                        break;
                    }
                }
            }

            let title = match title {
                Some(t) => t,
                None => continue,
            };

            // Collect all text lines for simple parsing
            let all_text: Vec<String> = doc
                .root_element()
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect();

            // Poster: prefer real film poster served from appalcinema.it
            let mut poster_url = None;
            if let Ok(img_sel) = Selector::parse("img[src]") {
                for img in doc.select(&img_sel) {
                    if let Some(src) = img.value().attr("src") {
                        let s = src.trim();
                        if s.contains("appalcinema.") {
                            poster_url = Some(s.to_string());
                            break;
                        }
                    }
                }
            }

            let mut regia = None;
            let mut attori = None;
            let mut running_time = None;
            let mut synopsis_parts = Vec::new();

            let mut after_duration = false;
            for line in &all_text {
                if line.starts_with("REGIA:") {
                    regia = Some(line.trim_start_matches("REGIA:").trim().to_string());
                } else if line.starts_with("ATTORI:") {
                    attori = Some(line.trim_start_matches("ATTORI:").trim().to_string());
                } else if line.starts_with("Durata:") {
                    let rest = line.trim_start_matches("Durata:").trim();
                    if let Some(min_str) = rest.split_whitespace().next() {
                        running_time = min_str.parse::<u32>().ok();
                    }
                    after_duration = true;
                } else if after_duration {
                    // Stop synopsis collection when we hit obvious non-synopsis markers
                    if line.starts_with("Sito ufficiale")
                        || line.starts_with("## ORARI")
                        || line.contains('/')
                    {
                        break;
                    }
                    // Skip menu/footer and very short lines
                    if line.len() > 40
                        && !line.contains("Home")
                        && !line.contains("Film della settimana")
                        && !line.contains("Il cinema")
                        && !line.contains("Info e costi")
                    {
                        synopsis_parts.push(line.clone());
                    }
                }
            }

            let cast = match (regia, attori) {
                (Some(r), Some(a)) => Some(format!("Regia: {}. Attori: {}", r, a)),
                (Some(r), None) => Some(format!("Regia: {}", r)),
                (None, Some(a)) => Some(format!("Attori: {}", a)),
                (None, None) => None,
            };

            let synopsis = if synopsis_parts.is_empty() {
                None
            } else {
                Some(synopsis_parts.join(" "))
            };

            films.push(Film {
                title,
                url: url.clone(),
                poster_url,
                cast,
                release_date: None,
                running_time,
                synopsis,
                showtimes: None, // listing page has showtimes, but they are omitted for simplicity
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/padova.xml".to_string()
    }
}
