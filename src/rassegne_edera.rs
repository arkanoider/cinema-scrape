use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Scraper for Cinema Edera rassegne (e.g. 10 E LUCE).
/// Treats each rassegna page as a "film" entry with long-form text and
/// also opens linked film pages to collect posters and short descriptions.
pub struct RassegneScraperEdera {
    url: String,
}

impl RassegneScraperEdera {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for RassegneScraperEdera {
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

        // Collect unique rassegna URLs like rassegne/10-e-luce.html
        // and, when available, their poster image URLs from the main page.
        let rassegna_links: Vec<(String, Option<String>)> = {
            let document = Html::parse_document(&body);
            // Match only the image links for each rassegna entry so we
            // reliably capture the poster banner, e.g.
            // <a href="/rassegne/10-e-luce.html" class="post__image-link">
            //   <img src="/_media/images/thumbs/1600x600_10_e_luce_banner.jpg" ...>
            // </a>
            let link_selector = Selector::parse("a.post__image-link[href*=\"rassegne/\"]")?;
            let img_selector = Selector::parse("img")?;
            let mut links = Vec::new();
            let mut seen = HashSet::new();

            for a in document.select(&link_selector) {
                if let Some(href) = a.value().attr("href") {
                    let href = href.trim();
                    if href.is_empty() {
                        continue;
                    }
                    let full_url = if href.starts_with("http") {
                        href.to_string()
                    } else {
                        format!("https://www.cinemaedera.it{}", href)
                    };
                    if seen.insert(full_url.clone()) {
                        // Try to grab a poster image inside the link, if present.
                        let poster_url = a
                            .select(&img_selector)
                            .next()
                            .and_then(|img| img.value().attr("src"))
                            .map(|src| {
                                let src = src.trim();
                                if src.starts_with("http") {
                                    src.to_string()
                                } else {
                                    format!("https://www.cinemaedera.it{}", src)
                                }
                            });

                        links.push((full_url, poster_url));
                    }
                }
            }

            links
        };

        if rassegna_links.is_empty() {
            return Ok(Vec::new());
        }

        let mut films = Vec::new();

        for (url, poster_url) in rassegna_links {
            let resp = client
                .get(&url)
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
            let doc = Html::parse_document(&body);

            // Title from the page heading, e.g. <h2 class="page-heading">10 E LUCE</h2>
            let title_selector = Selector::parse("h2.page-heading")?;
            let title = doc
                .select(&title_selector)
                .next()
                .map(|h2| {
                    h2.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| url.clone());

            // Date range line: text starting with "Dal ...".
            let date_range = {
                let text_nodes: Vec<String> = doc
                    .root_element()
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .map(|t| t.to_string())
                    .collect();
                text_nodes.iter().find(|s| s.starts_with("Dal ")).cloned()
            };

            // All <h3> blocks on the page are the per-film descriptions we care about.
            let h3_selector = Selector::parse("h3")?;
            let mut entries = Vec::new();
            for h3 in doc.select(&h3_selector) {
                let text = h3
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !text.is_empty() {
                    entries.push(text);
                }
            }

            // If there are no <h3> entries at all (e.g. flyer/ABC page), skip.
            if entries.is_empty() {
                continue;
            }

            // Build a simple long-form synopsis:
            // - Cinema name
            // - Date range line (Dal ... al ...)
            // - Bullet-like list of all <h3> entries.
            let synopsis = {
                let mut parts = Vec::new();
                parts.push("Cinema: Cinema Edera".to_string());
                if let Some(ds) = &date_range {
                    parts.push(ds.clone());
                }
                if !entries.is_empty() {
                    parts.push("I film della rassegna:".to_string());
                    for e in entries {
                        parts.push(format!("* {}", e));
                    }
                }
                Some(parts.join("\n\n"))
            };

            films.push(Film {
                title,
                url,
                poster_url,
                cast: None,
                release_date: date_range,
                running_time: None,
                synopsis,
                showtimes: None,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "rassegne_edera.xml".to_string()
    }
}
