use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Scraper for Cinema Cristallo Oderzo "Rassegna Film d’Autore".
/// Starts from the rassegna listing page and follows each film link.
pub struct RassegneScraper {
    url: String,
}

impl RassegneScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for RassegneScraper {
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

        // Collect unique film URLs from the Rassegna section.
        // We scope to the amy-section row used on the Rassegna page
        // to avoid picking up navigation/menu links.
        let film_urls: Vec<String> = {
            let document = Html::parse_document(&body);
            let section_selector =
                Selector::parse("div.amy-section.wpb_row.vc_custom_1666775304691")?;
            let link_selector = Selector::parse("a[href*=\"/movie/\"]")?;

            let mut urls = Vec::new();
            let mut seen = HashSet::new();

            for section in document.select(&section_selector) {
                for a in section.select(&link_selector) {
                    if let Some(href) = a.value().attr("href") {
                        let href = href.trim();
                        if href.is_empty() {
                            continue;
                        }
                        let full = if href.starts_with("http") {
                            href.to_string()
                        } else {
                            format!("https://www.cinemacristallo.com{}", href)
                        };
                        if seen.insert(full.clone()) {
                            urls.push(full);
                        }
                    }
                }
            }

            urls
        };

        if film_urls.is_empty() {
            return Ok(Vec::new());
        }

        // For each film page, extract:
        // - side column block (data, genere, durata)
        // - poster image
        // - long-form synopsis / description
        let info_container_selector =
            Selector::parse("div.row.amy-single-movie div.col-md-4.col-sm-4")?;
        let poster_selector = Selector::parse("div.row.amy-single-movie img")?;

        let mut films = Vec::new();

        for url in film_urls {
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

            let container = match doc.select(&info_container_selector).next() {
                Some(c) => c,
                None => {
                    // If layout is unexpected, fall back to using <h1> as title only.
                    let title = extract_title_fallback(&doc).unwrap_or_else(|| url.clone());
                    films.push(Film {
                        title,
                        url,
                        poster_url: extract_poster(&doc, &poster_selector),
                        cast: None,
                        release_date: None,
                        running_time: None,
                        synopsis: extract_synopsis(&doc),
                        showtimes: None,
                    });
                    continue;
                }
            };

            let text_lines: Vec<String> = container
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect();

            let mut title: Option<String> = None;
            let mut release_date: Option<String> = None;
            let mut running_time: Option<u32> = None;
            let mut genre: Option<String> = None;

            for line in &text_lines {
                let lower = line.to_lowercase();

                // First non-label line is the title fallback if we don't find a better one.
                if title.is_none()
                    && !lower.starts_with("data uscita")
                    && !lower.starts_with("durata")
                    && !lower.starts_with("genere")
                {
                    title = Some(line.clone());
                }

                if lower.starts_with("data uscita") {
                    if let Some((_, rest)) = line.split_once(':') {
                        let value = rest.trim();
                        if !value.is_empty() {
                            release_date = Some(value.to_string());
                        }
                    }
                } else if lower.starts_with("durata") {
                    // Example: "Durata: 01 ore 42 minuti"
                    if let Some((_, rest)) = line.split_once(':') {
                        let tokens: Vec<&str> = rest.split_whitespace().collect();
                        let mut hours: u32 = 0;
                        let mut minutes: u32 = 0;
                        for (idx, tok) in tokens.iter().enumerate() {
                            if let Ok(n) = tok.parse::<u32>() {
                                if idx + 1 < tokens.len() && tokens[idx + 1].starts_with("ore") {
                                    hours = n;
                                } else if idx + 1 < tokens.len()
                                    && tokens[idx + 1].starts_with("min")
                                {
                                    minutes = n;
                                }
                            }
                        }
                        let total = hours.saturating_mul(60).saturating_add(minutes);
                        if total > 0 {
                            running_time = Some(total);
                        }
                    }
                } else if lower.starts_with("genere") {
                    if let Some((_, rest)) = line.split_once(':') {
                        let value = rest.trim();
                        if !value.is_empty() {
                            genre = Some(value.to_string());
                        }
                    }
                }
            }

            // If we did not manage to find a title inside the info block,
            // fall back to <h1> from the page.
            let title = title
                .or_else(|| extract_title_fallback(&doc))
                .unwrap_or_else(|| url.clone());

            let cast = genre.as_ref().map(|g| format!("Genere: {}", g));

            let poster_url = extract_poster(&doc, &poster_selector);
            let synopsis = extract_synopsis(&doc);

            films.push(Film {
                title,
                url,
                poster_url,
                cast,
                release_date,
                running_time,
                synopsis,
                showtimes: None,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "rassegne.xml".to_string()
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

/// Extract poster URL from the single-movie layout.
fn extract_poster(doc: &Html, poster_selector: &Selector) -> Option<String> {
    let base = "https://www.cinemacristallo.com";
    if let Some(img) = doc.select(poster_selector).next()
        && let Some(src) = img.value().attr("src")
    {
        let src = src.trim();
        if src.is_empty() {
            return None;
        }
        return Some(if src.starts_with("http") {
            src.to_string()
        } else {
            format!("{}{}", base, src)
        });
    }
    None
}

/// Extract a textual synopsis / description from the film page.
fn extract_synopsis(doc: &Html) -> Option<String> {
    // Try a few likely containers used by the Amy cinema theme.
    let candidates = [
        "div.amy-single-movie div.entry-content p",
        "div.amy-single-movie div.amy-single-movie-content p",
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
