//! Scraper for Cinergia Conegliano (18tickets platform).
//! Listing: https://coneglianocinergia.18tickets.it/
//! Film page: https://coneglianocinergia.18tickets.it/film/41324?ref_date=YYYY-MM-DD

use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// Fallback: find film IDs in raw HTML/JSON (e.g. data attributes or inline script).
fn extract_film_ids_from_raw(html: &str) -> Vec<String> {
    let mut ids = HashSet::new();
    for (i, _) in html.match_indices("/film/") {
        let after = &html[i + 6..]; // after "/film/"
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let id = &after[..end];
        if !id.is_empty() {
            ids.insert(id.to_string());
        }
    }
    let mut v: Vec<String> = ids.into_iter().collect();
    v.sort();
    v
}

/// Returns unique film IDs from listing: links like /film/41324 or https://..../film/41324?...
fn extract_film_ids(document: &Html) -> Vec<String> {
    let link_sel = match Selector::parse("a[href*=\"/film/\"]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut ids = HashSet::new();
    for a in document.select(&link_sel) {
        let href = match a.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        let path = href.split('?').next().unwrap_or(href);
        // Match /film/NUMBER (relative) or .../film/NUMBER (absolute); skip /film/41324/uuid
        let rest = if path.starts_with("/film/") {
            path.trim_start_matches("/film/").trim_end_matches('/')
        } else if path.contains("/film/") {
            path.split("/film/")
                .nth(1)
                .unwrap_or("")
                .trim_end_matches('/')
        } else {
            continue;
        };
        let id_part = rest.split('/').next().unwrap_or(rest);
        if !id_part.is_empty() && id_part.chars().all(|c| c.is_ascii_digit()) {
            ids.insert(id_part.to_string());
        }
    }
    let mut v: Vec<String> = ids.into_iter().collect();
    v.sort();
    v
}

/// Scraper for Cinergia Conegliano (18tickets).
pub struct CinergiaConeglianoScraper {
    base_url: String,
}

impl CinergiaConeglianoScraper {
    pub fn new(base_url: String) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self { base_url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for CinergiaConeglianoScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(self.base_url.as_str())
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;

        let film_ids = {
            let document = Html::parse_document(&body);
            let from_doc = extract_film_ids(&document);
            if from_doc.is_empty() {
                // Fallback: 18tickets may inject links via JS; scrape /film/NUMBER from raw HTML
                extract_film_ids_from_raw(&body)
            } else {
                from_doc
            }
        };

        if film_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Use today for ref_date so film page shows current week showtimes
        let ref_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut films = Vec::new();

        for id in film_ids {
            let film_url = format!("{}/film/{}?ref_date={}", self.base_url, id, ref_date);
            let resp = match client
                .get(&film_url)
                .header(header::USER_AGENT, USER_AGENT)
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

            // Title: first h1, h2, h3, h4, h5, h6 with content
            let title = {
                let h_sel = Selector::parse("h1, h2, h3, h4, h5, h6").ok();
                let mut t = None;
                if let Some(ref sel) = h_sel {
                    for h in doc.select(sel) {
                        let text = h
                            .text()
                            .map(|x| x.trim())
                            .filter(|x| !x.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !text.is_empty()
                            && !text.eq_ignore_ascii_case("Plot")
                            && !text.eq_ignore_ascii_case("Info")
                            && !text.eq_ignore_ascii_case("Trama")
                        {
                            t = Some(text);
                            break;
                        }
                    }
                }
                t.unwrap_or_else(|| format!("Film {}", id))
            };

            // Poster: og:image first, then first img with substantial src
            let poster_url = Selector::parse("meta[property=\"og:image\"]")
                .ok()
                .and_then(|sel| {
                    doc.select(&sel)
                        .next()
                        .and_then(|m| m.value().attr("content").map(String::from))
                })
                .or_else(|| {
                    Selector::parse("img[src]").ok().and_then(|sel| {
                        doc.select(&sel).find_map(|img| {
                            let src = img.value().attr("src")?;
                            let s = src.trim();
                            if s.starts_with("http") && !s.contains("cookie") && !s.contains("logo")
                            {
                                Some(s.to_string())
                            } else {
                                None
                            }
                        })
                    })
                });

            // Flatten text for line-by-line parsing
            let all_text: Vec<String> = doc
                .root_element()
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect();

            let mut running_time = None;
            let mut director = None;
            let mut with_cast = None;
            let mut synopsis_parts = Vec::new();
            let mut showtimes = Vec::new();
            let mut in_plot = false;
            let mut current_date_line: Option<String> = None;

            fn looks_like_time(s: &str) -> bool {
                let s = s.trim().trim_start_matches('-').trim();
                if s.len() >= 4 && s.contains(':') {
                    let parts: Vec<&str> = s.split(':').collect();
                    parts.len() == 2
                        && parts[0].chars().all(|c| c.is_ascii_digit())
                        && parts[1].chars().all(|c| c.is_ascii_digit())
                } else {
                    false
                }
            }
            fn looks_like_date_line(s: &str) -> bool {
                (s.contains('/') && s.contains("2026"))
                    || (s.contains("February") || s.contains("Febbraio"))
                        && (s.contains("Friday")
                            || s.contains("Saturday")
                            || s.contains("Sunday")
                            || s.contains("Monday")
                            || s.contains("Tuesday")
                            || s.contains("Wednesday")
                            || s.contains("Thursday")
                            || s.contains("Venerdì")
                            || s.contains("Sabato")
                            || s.contains("Domenica")
                            || s.contains("Lunedì")
                            || s.contains("Martedì")
                            || s.contains("Mercoledì")
                            || s.contains("Giovedì"))
            }

            for (i, line) in all_text.iter().enumerate() {
                if line.starts_with("Durata:") {
                    let rest = line.trim_start_matches("Durata:").trim();
                    if let Some(num_str) = rest.split_whitespace().next() {
                        running_time = num_str.parse::<u32>().ok();
                    }
                } else if line.eq_ignore_ascii_case("Director:") {
                    if let Some(next) = all_text.get(i + 1) {
                        director = Some(next.clone());
                    }
                } else if line.eq_ignore_ascii_case("With:") || line.eq_ignore_ascii_case("Con:") {
                    if let Some(next) = all_text.get(i + 1) {
                        with_cast = Some(next.clone());
                    }
                } else if line.eq_ignore_ascii_case("Plot") || line.eq_ignore_ascii_case("Trama") {
                    in_plot = true;
                } else if in_plot {
                    if line.eq_ignore_ascii_case("Info") || looks_like_date_line(line) {
                        in_plot = false;
                        if looks_like_date_line(line) {
                            current_date_line = Some(line.clone());
                        }
                    } else if line.len() > 30
                        && !line.contains("Watch the trailer")
                        && !line.contains("Seleziona")
                        && !line.contains("Select ")
                    {
                        synopsis_parts.push(line.clone());
                    }
                }

                if looks_like_date_line(line) {
                    current_date_line = Some(line.clone());
                } else if looks_like_time(line) {
                    let time = line.trim().trim_start_matches('-').trim();
                    if let Some(ref date) = current_date_line {
                        showtimes.push(format!("{} ore {}", date, time));
                    } else {
                        showtimes.push(format!("ore {}", time));
                    }
                }
            }

            let cast = match (director.as_ref(), with_cast.as_ref()) {
                (Some(d), Some(w)) => Some(format!("Regia: {}. Con: {}", d, w)),
                (Some(d), None) => Some(format!("Regia: {}", d)),
                (None, Some(w)) => Some(format!("Con: {}", w)),
                (None, None) => None,
            };
            let synopsis = if synopsis_parts.is_empty() {
                None
            } else {
                Some(synopsis_parts.join("\n\n"))
            };
            let showtimes = if showtimes.is_empty() {
                None
            } else {
                Some(showtimes)
            };

            films.push(Film {
                title,
                url: film_url,
                poster_url,
                cast,
                release_date: None,
                running_time,
                synopsis,
                showtimes,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/cinergia_conegliano.xml".to_string()
    }
}
