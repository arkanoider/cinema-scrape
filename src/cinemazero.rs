use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};

/// Scraper for Cinemazero (homepage "Il programma di oggi" section).
// Fetches daily program and parses detail pages.
pub struct CinemazeroScraper {
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
        // 1) Fetch homepage and collect unique film URLs from any link
        //    pointing to /film/... . This naturally reflects today's
        //    programme ("Oggi al Cinema" / "Il programma di oggi").
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
        let film_urls: Vec<String> = {
            let document = Html::parse_document(&body);
            let link_selector =
                Selector::parse("a[href]").map_err(|e| format!("selector error: {e}"))?;
            let mut film_urls: Vec<String> = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for a in document.select(&link_selector) {
                if let Some(href) = a.value().attr("href")
                    && href.contains("/film/")
                {
                    let absolute = if href.starts_with("http") {
                        href.to_string()
                    } else {
                        format!("https://cinemazero.it{}", href)
                    };
                    if seen.insert(absolute.clone()) {
                        film_urls.push(absolute);
                    }
                }
            }

            film_urls
        };

        if film_urls.is_empty() {
            return Ok(Vec::new());
        }

        // 2) For each film URL, open its detail page and extract
        //    title, synopsis, metadata and showtimes.
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

            // Poster: <img ... alt="Immagine del film La grazia"> etc.
            let mut poster_url: Option<String> = None;
            if let Ok(img_sel) = Selector::parse("img[alt*=\"Immagine del film\"]")
                && let Some(img) = doc.select(&img_sel).next()
                && let Some(src) = img.value().attr("src")
                && !src.trim().is_empty()
            {
                poster_url = Some(src.to_string());
            }

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

            for s in all_text.iter().skip(title_idx + 1) {
                let lower = s.to_lowercase();

                // Stop if we hit "Programmazione e orari"
                if lower.contains("programmazione e orari") {
                    break;
                }

                // Metadata headings: Genere / Regia / Cast
                if lower.starts_with("genere ")
                    || lower.starts_with("regia ")
                    || lower.starts_with("cast")
                {
                    if lower.starts_with("genere ") {
                        genere = Some(
                            s["Genere".len()..]
                                .trim_matches(|c: char| c == ':' || c.is_whitespace())
                                .to_string(),
                        );
                    } else if lower.starts_with("regia ") {
                        regia = Some(
                            s["Regia".len()..]
                                .trim_matches(|c: char| c == ':' || c.is_whitespace())
                                .to_string(),
                        );
                    } else {
                        // Handle both "Cast " and "Cast  Foo..."
                        let after = s.trim_start_matches("Cast").trim_start_matches(':').trim();
                        if !after.is_empty() {
                            cast_line = Some(after.to_string());
                        }
                    }
                    continue;
                }

                // Anything between title and metadata headings is likely synopsis.
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
                        showtimes.push(entry);
                    }
                }
            }

            films.push(Film {
                title,
                url,
                poster_url,
                cast,
                release_date: None,
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
