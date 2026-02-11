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

        // Collect unique rassegna URLs like /rassegne/10-e-luce.html
        let rassegna_urls: Vec<String> = {
            let document = Html::parse_document(&body);
            let link_selector = Selector::parse("a[href*=\"/rassegne/\"]")?;
            let mut urls = Vec::new();
            let mut seen = HashSet::new();

            for a in document.select(&link_selector) {
                if let Some(href) = a.value().attr("href") {
                    let href = href.trim();
                    if href.is_empty() {
                        continue;
                    }
                    // Skip the listing page itself if it appears as a link.
                    if href.ends_with("/rassegne.html") {
                        continue;
                    }
                    let full = if href.starts_with("http") {
                        href.to_string()
                    } else {
                        format!("https://www.cinemaedera.it{}", href)
                    };
                    if seen.insert(full.clone()) {
                        urls.push(full);
                    }
                }
            }

            urls
        };

        if rassegna_urls.is_empty() {
            return Ok(Vec::new());
        }

        let mut films = Vec::new();

        for url in rassegna_urls {
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

            // Scope HTML parsing so non-Send types are dropped before awaits
            // when fetching inner film pages.
            let (title, date_range, synopsis_raw, inner_urls): (
                String,
                Option<String>,
                Option<String>,
                Vec<String>,
            ) = {
                let doc = Html::parse_document(&body);

                // Title: try main heading on the page.
                let title = extract_title_fallback(&doc).unwrap_or_else(|| url.clone());

                // Date range line: something starting with "Dal".
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

                // Long-form description: rassegna page text.
                let synopsis_raw = extract_synopsis(&doc);

                // Collect inner film links inside this rassegna page.
                let inner_link_selector = Selector::parse("a[href]")
                    .map_err(|e| format!("selector error: {e}"))?;
                let mut inner_urls: Vec<String> = Vec::new();
                let mut seen_inner = HashSet::new();

                for a in doc.select(&inner_link_selector) {
                    if let Some(href) = a.value().attr("href") {
                        let href = href.trim();
                        if href.is_empty() {
                            continue;
                        }
                        // Skip links that point to other rassegna pages or navigation.
                        if href.contains("/rassegne/") {
                            continue;
                        }
                        // Heuristic: keep only links that look like film detail pages.
                        if !(href.contains("/film") || href.contains("i-film")) {
                            continue;
                        }
                        let full = if href.starts_with("http") {
                            href.to_string()
                        } else {
                            format!("https://www.cinemaedera.it{}", href)
                        };
                        if seen_inner.insert(full.clone()) {
                            inner_urls.push(full);
                        }
                    }
                }

                (title, date_range, synopsis_raw, inner_urls)
            };

            // Now fetch each inner film page without holding onto `doc`.
            let inner_poster_selector = Selector::parse(".movie__images img.img-responsive")
                .map_err(|e| format!("selector error: {e}"))?;
            let inner_desc_selector =
                Selector::parse("p.movie__describe").map_err(|e| format!("selector error: {e}"))?;

            let mut inner_infos: Vec<(String, Option<String>, Option<String>)> = Vec::new();

            for full in inner_urls {
                let resp = client
                    .get(&full)
                    .header(
                        header::USER_AGENT,
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                         AppleWebKit/537.36 (KHTML, like Gecko) \
                         Chrome/143.0.0.0 Safari/537.36",
                    )
                    .send()
                    .await?
                    .error_for_status()?;
                let film_body = resp.text().await?;
                let film_doc = Html::parse_document(&film_body);

                let film_title =
                    extract_title_fallback(&film_doc).unwrap_or_else(|| full.clone());

                // Poster from the standard Edera film layout.
                let film_poster = film_doc
                    .select(&inner_poster_selector)
                    .next()
                    .and_then(|img| img.value().attr("src"))
                    .map(|src| src.trim())
                    .filter(|src| !src.is_empty())
                    .map(|src| {
                        if src.starts_with("http") {
                            src.to_string()
                        } else {
                            format!("https://www.cinemaedera.it{}", src)
                        }
                    });

                // Short synopsis from p.movie__describe if available.
                let film_synopsis = film_doc
                    .select(&inner_desc_selector)
                    .next()
                    .map(|p| {
                        p.text()
                            .map(|t| t.trim())
                            .filter(|t| !t.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .filter(|s| !s.is_empty());

                inner_infos.push((film_title, film_poster, film_synopsis));
            }

            let synopsis = {
                let mut parts = Vec::new();
                parts.push("Cinema: Cinema Edera".to_string());
                if let Some(ds) = &date_range {
                    parts.push(ds.clone());
                }
                if let Some(text) = synopsis_raw {
                    parts.push(text);
                }
                if !inner_infos.is_empty() {
                    parts.push("I film della rassegna:".to_string());
                    for (film_title, _, film_synopsis) in &inner_infos {
                        let mut block = format!("* {}", film_title);
                        if let Some(s) = film_synopsis {
                            block.push('\n');
                            block.push_str(s);
                        }
                        parts.push(block);
                    }
                }
                Some(parts.join("\n\n"))
            };

            // Use the first inner film poster (if any) as the rassegna poster.
            let poster_url = inner_infos
                .iter()
                .find_map(|(_, poster, _)| poster.clone());

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

/// Fallback title extraction from a generic <h1>.
fn extract_title_fallback(doc: &Html) -> Option<String> {
    let h1_selector = Selector::parse("h1").ok()?;
    doc.select(&h1_selector)
        .next()
        .map(|h1| {
            h1.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
}

/// Extract a textual synopsis / description from a rassegna page.
fn extract_synopsis(doc: &Html) -> Option<String> {
    let candidates = [
        "#main-content-wrapper section p",
        "div.entry-content p",
        "article div.entry-content p",
    ];

    for sel_str in &candidates {
        let selector = match Selector::parse(sel_str) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut parts = Vec::new();
        for p in doc.select(&selector) {
            let text = p
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                parts.push(text);
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n\n"));
        }
    }

    None
}

