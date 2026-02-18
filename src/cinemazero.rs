use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

const PROGRAMMAZIONE_URL: &str = "https://cinemazero.it/programmazione/";
const CINEMAZERO_FILM_PREFIX: &str = "https://cinemazero.it/film/";

/// Scraper for Cinemazero. Fetches the programmazione listing, collects film detail URLs,
/// then opens each film page to extract poster, synopsis, cast, regia and durata.
pub struct CinemazeroScraper {
    #[allow(dead_code)]
    url: String,
}

impl CinemazeroScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for CinemazeroScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        // 1) Fetch programmazione listing and collect unique film detail URLs.
        //    Only links to cinemazero.it/film/... (exclude 18tickets, etc.).
        let resp = client
            .get(PROGRAMMAZIONE_URL)
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
        let film_urls: Vec<String> = {
            let document = Html::parse_document(&body);
            let link_selector =
                Selector::parse("a[href]").map_err(|e| format!("selector error: {e}"))?;
            let mut seen: HashSet<String> = HashSet::new();
            let mut list: Vec<String> = Vec::new();

            for a in document.select(&link_selector) {
                let href = match a.value().attr("href") {
                    Some(h) => h.trim(),
                    None => continue,
                };
                if !href.contains("/film/") {
                    continue;
                }
                let absolute = if href.starts_with("http") {
                    href.to_string()
                } else if href.starts_with('/') {
                    format!("https://cinemazero.it{}", href)
                } else {
                    format!("https://cinemazero.it/{}", href)
                };
                // Only cinemazero.it film pages (exclude 18tickets, multisala, etc.)
                if absolute.starts_with(CINEMAZERO_FILM_PREFIX) && seen.insert(absolute.clone()) {
                    list.push(absolute);
                }
            }

            list
        };

        if film_urls.is_empty() {
            return Ok(Vec::new());
        }

        // 2) Open each film detail page and extract poster_url, sinossi, cast, regia, durata, showtimes.
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

            // Poster: <img ... alt="Immagine del film ..." src="..."> (may be relative or absolute)
            let mut poster_url: Option<String> = None;
            if let Ok(img_sel) = Selector::parse("img[alt*=\"Immagine del film\"]")
                && let Some(img) = doc.select(&img_sel).next()
                && let Some(src) = img.value().attr("src")
            {
                let s = src.trim();
                if !s.is_empty() {
                    poster_url = Some(if s.starts_with("http") {
                        s.to_string()
                    } else if s.starts_with('/') {
                        format!("https://cinemazero.it{s}")
                    } else {
                        format!("https://cinemazero.it/{s}")
                    });
                }
            }

            // Uscita (release year): <span aria-label="Uscita">2025</span>
            let release_date: Option<String> = Selector::parse("span[aria-label=\"Uscita\"]")
                .ok()
                .and_then(|sel| doc.select(&sel).next())
                .and_then(|span| {
                    let t: String = span
                        .text()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if t.is_empty() { None } else { Some(t) }
                });

            // Collect all non-empty text nodes, in order, so we can parse
            // sections like "Genere", "Regia", "Cast", "Programmazione e orari".
            let all_text: Vec<String> = doc
                .root_element()
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect();

            // Title: try <h1>, fall back to first line, fall back to URL.
            let h1_selector = Selector::parse("h1").map_err(|e| format!("selector error: {e}"))?;
            let mut title = doc
                .select(&h1_selector)
                .next()
                .map(|h1| {
                    h1.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            if title.is_empty() {
                if let Some(first) = all_text.first() {
                    title = first.clone();
                } else {
                    title = url.clone();
                }
            }

            // Find index of the title in the linearised text, so we can
            // treat following lines up to "Genere" as synopsis.
            let title_idx = all_text
                .iter()
                .position(|s| s.eq_ignore_ascii_case(&title))
                .unwrap_or(0);

            let mut synopsis_lines: Vec<String> = Vec::new();
            let mut genere: Option<String> = None;
            let mut regia: Option<String> = None;
            let mut cast_line: Option<String> = None;

            // First pass: find Genere, Regia, Cast in the whole text (they are often on their own
            // line with the value on the next line, or "Label value" on one line).
            for (i, s) in all_text.iter().enumerate() {
                let trimmed = s.trim();
                let label = trimmed.to_lowercase();
                if label == "genere" {
                    if let Some(next) = all_text.get(i + 1) {
                        let val = next.trim();
                        if !val.is_empty() && !val.to_lowercase().starts_with("regia") {
                            genere = Some(val.to_string());
                        }
                    }
                    break;
                }
                if label.starts_with("genere ") {
                    let val = trimmed["Genere".len()..]
                        .trim_matches(|c: char| c == ':' || c.is_whitespace());
                    if !val.is_empty() {
                        genere = Some(val.to_string());
                    }
                    break;
                }
            }
            for (i, s) in all_text.iter().enumerate() {
                let trimmed = s.trim();
                let label = trimmed.to_lowercase();
                if label == "regia" {
                    if let Some(next) = all_text.get(i + 1) {
                        let val = next.trim();
                        if !val.is_empty() && !val.to_lowercase().starts_with("cast") {
                            regia = Some(val.to_string());
                        }
                    }
                    break;
                }
                if label.starts_with("regia ") {
                    let val = trimmed["Regia".len()..]
                        .trim_matches(|c: char| c == ':' || c.is_whitespace());
                    if !val.is_empty() {
                        regia = Some(val.to_string());
                    }
                    break;
                }
            }
            for (i, s) in all_text.iter().enumerate() {
                let trimmed = s.trim();
                let label = trimmed.to_lowercase();
                if label == "cast" {
                    if let Some(next) = all_text.get(i + 1) {
                        let val = next.trim();
                        if !val.is_empty() && val.len() > 2 {
                            cast_line = Some(val.to_string());
                        }
                    }
                    break;
                }
                if label.starts_with("cast") && trimmed.len() > 4 {
                    let after = trimmed["Cast".len()..]
                        .trim_start_matches(|c: char| c == ':' || c.is_whitespace());
                    if !after.is_empty() {
                        cast_line = Some(after.to_string());
                    }
                    break;
                }
            }

            // Synopsis: text between title and "Genere" (or first metadata).
            for s in all_text.iter().skip(title_idx + 1) {
                let lower = s.to_lowercase();
                if lower.contains("programmazione e orari") {
                    break;
                }
                if s.trim().to_lowercase() == "genere"
                    || s.trim().to_lowercase() == "regia"
                    || s.trim().to_lowercase() == "cast"
                    || s.trim().to_lowercase().starts_with("genere ")
                    || s.trim().to_lowercase().starts_with("regia ")
                    || s.trim().to_lowercase().starts_with("cast ")
                {
                    break;
                }
                synopsis_lines.push(s.clone());
            }

            let mut synopsis = if synopsis_lines.is_empty() {
                None
            } else {
                Some(synopsis_lines.join(" "))
            };

            // Clean synopsis of control characters if present
            if let Some(ref mut s) = synopsis {
                *s = s.chars().filter(|c| !c.is_control()).collect();
            }

            // Fallback: if we failed to detect a synopsis from the linear text,
            // pick the longest <p> that looks like a plot (long text, with punctuation),
            // excluding obvious metadata blocks.
            if synopsis.is_none()
                && let Ok(p_sel) = Selector::parse("p")
            {
                let mut best: Option<String> = None;
                let mut best_len: usize = 0;
                for p in doc.select(&p_sel) {
                    let text = p
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let lower = text.to_lowercase();
                    let len = text.len();
                    if len < 80 {
                        continue;
                    }
                    if lower.contains("genere")
                        || lower.contains("regia")
                        || lower.contains("cast")
                        || lower.contains("programmazione e orari")
                    {
                        continue;
                    }
                    if !lower.contains('.') {
                        continue;
                    }
                    if len > best_len {
                        best_len = len;
                        best = Some(text);
                    }
                }
                if let Some(text) = best {
                    // Clean text of control characters
                    let clean_text: String = text.chars().filter(|c| !c.is_control()).collect();
                    synopsis = Some(clean_text);
                }
            }

            // Build a compact "cast" field combining genre, regia and cast.
            let mut cast_parts = Vec::new();
            if let Some(g) = genere.clone() {
                cast_parts.push(format!("Genere: {}", g));
            }
            if let Some(r) = regia.clone() {
                cast_parts.push(format!("Regia: {}", r));
            }
            if let Some(c) = cast_line.clone() {
                cast_parts.push(format!("Cast: {}", c));
            }
            let cast = if cast_parts.is_empty() {
                None
            } else {
                Some(cast_parts.join(" | "))
            };

            // Running time in minutes: look for a short line ending with "m" or "min".
            let mut running_time: Option<u32> = None;
            for s in &all_text {
                let lower = s.to_lowercase();
                if (lower.ends_with(" m") || lower.ends_with(" min"))
                    && let Some(num_str) = s.split_whitespace().next()
                    && let Ok(n) = num_str.parse::<u32>()
                {
                    running_time = Some(n);
                    break;
                }
            }

            // Showtimes: parse "Programmazione e orari" section.
            let mut showtimes: Vec<String> = Vec::new();
            if let Some(start_idx) = all_text
                .iter()
                .position(|s| s.to_lowercase().contains("programmazione e orari"))
            {
                let mut current_date: Option<String> = None;
                for s in all_text.iter().skip(start_idx + 1) {
                    let lower = s.to_lowercase();
                    if lower.starts_with("oggi al cinema") {
                        break;
                    }

                    // Heuristic: short line with a digit and no ':' is a date like "10 Mar".
                    if s.len() <= 12 && s.chars().any(|c| c.is_ascii_digit()) && !s.contains(':') {
                        current_date = Some(s.clone());
                        continue;
                    }

                    // Look for a time token like "16:00" and optional hall code.
                    let tokens: Vec<&str> = s.split_whitespace().collect();
                    if tokens.is_empty() {
                        continue;
                    }

                    let time_token = tokens.iter().copied().find(|t| t.contains(':'));
                    if let Some(time) = time_token
                        && let Some(ref date) = current_date
                    {
                        let hall = tokens
                            .iter()
                            .copied()
                            .find(|t| t.chars().all(|c| c.is_ascii_alphabetic()) && t.len() <= 4);
                        let mut entry = String::new();
                        entry.push_str(date);
                        if let Some(h) = hall {
                            entry.push(' ');
                            entry.push_str(h);
                        }
                        entry.push(' ');
                        entry.push_str(time);
                        // Skip false positives from synopsis (e.g. "2025 In secolare:")
                        if !entry.to_lowercase().contains("secolare") {
                            showtimes.push(entry);
                        }
                    }
                }
            }

            films.push(Film {
                title,
                url,
                poster_url,
                cast,
                release_date,
                running_time,
                synopsis,
                showtimes: if showtimes.is_empty() {
                    None
                } else {
                    Some(showtimes)
                },
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/cinemazero.xml".to_string()
    }
}
