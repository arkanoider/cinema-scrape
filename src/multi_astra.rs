//! Scraper for Multi Astra Padova.
//! Listing: https://multiastra.it/film-della-settimana/
//! Film page: https://multiastra.it/film/barry-lyndon (title, poster, regia, cast, genere, durata, sinossi, orari)

use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

const LISTING_URL: &str = "https://multiastra.it/film-della-settimana/";
const BASE: &str = "https://multiastra.it";

fn is_day_line(line: &str) -> bool {
    let s = line.trim().trim_matches('*').trim();
    let day_prefixes = [
        "Domenica ",
        "Lunedì ",
        "Martedì ",
        "Mercoledì ",
        "Giovedì ",
        "Venerdì ",
        "Sabato ",
    ];
    day_prefixes.iter().any(|p| s.starts_with(p)) && s.len() <= 25 && s.contains('/')
}

/// Extract time tokens from a line, e.g. "20.00V.O.S", "15.3017.3020.30" -> ["20.00"], ["15.30","17.30","20.30"]
fn parse_time_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = line.as_bytes();
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            if i + 2 <= b.len() && b[i].is_ascii_digit() && b[i + 1].is_ascii_digit() {
                i += 2;
                let token = String::from_utf8_lossy(&b[start..i]).to_string();
                if token.len() >= 4 && token.len() <= 5 {
                    out.push(token);
                }
            }
        }
    }
    out
}

/// Scraper for Multi Astra Padova.
pub struct MultiAstraScraper {
    #[allow(dead_code)]
    listing_url: String,
}

impl MultiAstraScraper {
    pub fn new(listing_url: String) -> Self {
        Self { listing_url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for MultiAstraScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(LISTING_URL)
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;

        let urls: HashSet<String> = {
            let listing = Html::parse_document(&body);
            let link_sel = Selector::parse("a[href*=\"/film/\"]")?;
            let mut urls = HashSet::new();
            for a in listing.select(&link_sel) {
                let href = match a.value().attr("href") {
                    Some(h) => h.trim(),
                    None => continue,
                };
                if href.is_empty() {
                    continue;
                }
                let lower = href.to_lowercase();
                if lower.contains("18tickets") || lower.contains("multiastra.18tickets") {
                    continue;
                }
                let full = if lower.starts_with("http") {
                    if lower.contains("multiastra.it/film/") {
                        href.to_string()
                    } else {
                        continue;
                    }
                } else if href.starts_with("/film/") {
                    format!("{}{}", BASE, href)
                } else {
                    continue;
                };
                let normalized = full
                    .replace("http://", "https://")
                    .replace("https://www.multiastra.it", BASE)
                    .replace("http://www.multiastra.it", BASE);
                urls.insert(normalized);
            }
            urls
        };

        if urls.is_empty() {
            return Ok(Vec::new());
        }

        let mut films = Vec::new();
        for url in urls {
            let resp = match client
                .get(&url)
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

            // Title: class "title" first (per user), then h1/h2/h3
            let mut title = None;
            if let Ok(title_sel) = Selector::parse(".title")
                && let Some(el) = doc.select(&title_sel).next()
            {
                let t = el
                    .text()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !t.is_empty() {
                    title = Some(t);
                }
            }
            if title.is_none()
                && let Ok(h_sel) = Selector::parse("h1, h2, h3")
            {
                for h in doc.select(&h_sel) {
                    let t = h
                        .text()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !t.is_empty() && !t.eq("ORARI") && !t.contains("Articoli") {
                        title = Some(t);
                        break;
                    }
                }
            }
            let title = match title {
                Some(t) => t,
                None => continue,
            };

            // Poster: og:image then first img
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
                            if s.starts_with("http")
                                && !s.contains("logo")
                                && !s.contains("cookie")
                                && !s.contains("astra_181")
                                && !s.contains("127.png")
                            {
                                Some(s.to_string())
                            } else {
                                None
                            }
                        })
                    })
                });

            let all_text: Vec<String> = doc
                .root_element()
                .text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect();

            let mut regia = None;
            let mut attori = None;
            let mut genere = None;
            let mut running_time = None;
            let mut synopsis_parts = Vec::new();
            let mut after_metadata = false;

            for (i, line) in all_text.iter().enumerate() {
                if line.starts_with("Regia:") {
                    let v = line.trim_start_matches("Regia:").trim();
                    let v = if v.is_empty() || v == "." {
                        all_text.get(i + 1).map(String::as_str).unwrap_or("").trim()
                    } else {
                        v
                    };
                    if !v.is_empty() && v != "." {
                        regia = Some(v.to_string());
                    }
                } else if line.starts_with("Attori:") {
                    let v = line.trim_start_matches("Attori:").trim();
                    let v = if v.is_empty() || v == "." {
                        all_text.get(i + 1).map(String::as_str).unwrap_or("").trim()
                    } else {
                        v
                    };
                    if !v.is_empty() && v != "." {
                        attori = Some(v.to_string());
                    }
                } else if line.starts_with("Genere:") {
                    let v = line.trim_start_matches("Genere:").trim();
                    let v = if v.is_empty() || v == "." {
                        all_text.get(i + 1).map(String::as_str).unwrap_or("").trim()
                    } else {
                        v
                    };
                    if !v.is_empty() && v != "." {
                        genere = Some(v.to_string());
                    }
                } else if line.starts_with("Durata:") {
                    let rest = line.trim_start_matches("Durata:").trim();
                    if let Some(num_str) = rest.split_whitespace().next() {
                        running_time = num_str.parse::<u32>().ok();
                    }
                    after_metadata = true;
                } else if after_metadata {
                    if line.eq("ORARI")
                        || line.contains("ALTRI FILM")
                        || line.contains("Articoli correlati")
                    {
                        break;
                    }
                    if line.starts_with("//") || line.contains("carica l'") {
                        break;
                    }
                    let clean = line.split(" //").next().unwrap_or(line).trim();
                    if clean.len() > 50
                        && !clean.contains("Sito ufficiale")
                        && !clean.contains("Nazionalità")
                        && !clean.contains("Distribuzione")
                        && !clean.contains("Home")
                        && !clean.contains("Film della settimana")
                        && !clean.contains("function ")
                        && !clean.contains("frame.htm")
                    {
                        synopsis_parts.push(clean.to_string());
                    }
                }
            }

            let cast = match (regia.as_ref(), attori.as_ref(), genere.as_ref()) {
                (Some(r), Some(a), Some(g)) => {
                    Some(format!("Regia: {}. Attori: {}. Genere: {}", r, a, g))
                }
                (Some(r), Some(a), None) => Some(format!("Regia: {}. Attori: {}", r, a)),
                (Some(r), None, Some(g)) => Some(format!("Regia: {}. Genere: {}", r, g)),
                (None, Some(a), Some(g)) => Some(format!("Attori: {}. Genere: {}", a, g)),
                (Some(r), None, None) => Some(format!("Regia: {}", r)),
                (None, Some(a), None) => Some(format!("Attori: {}", a)),
                (None, None, Some(g)) => Some(format!("Genere: {}", g)),
                (None, None, None) => None,
            };
            let cast = cast.filter(|s| {
                !s.contains("Regia: .") && !s.contains("Attori: .") && !s.eq("Genere: .")
            });

            let synopsis = if synopsis_parts.is_empty() {
                None
            } else {
                Some(synopsis_parts.join(" "))
            };

            // ORARI: day lines (Lunedì 16/03) and time lines (20.00 or 20.00V.O.S). Order can be time then day.
            let showtimes = {
                let orari_start = all_text.iter().position(|l| l.eq("ORARI"));
                let orari_end = all_text
                    .iter()
                    .position(|l| l.contains("ALTRI FILM") || l.contains("Articoli correlati"));
                let start = orari_start.unwrap_or(0);
                let end = orari_end.unwrap_or(all_text.len());
                let slice = &all_text[start..end];
                let mut showtimes_vec = Vec::new();
                let mut time_buf = Vec::new();
                let mut last_day: Option<String> = None;
                for line in slice {
                    if is_day_line(line) {
                        let day_clean = line.trim().trim_matches('*').trim().to_string();
                        if let Some(ref d) = last_day
                            && !time_buf.is_empty()
                        {
                            showtimes_vec.push(format!("{} ore {}", d, time_buf.join(", ")));
                        }
                        last_day = Some(day_clean);
                        time_buf.clear();
                    } else {
                        for t in parse_time_tokens(line) {
                            time_buf.push(t);
                        }
                    }
                }
                if let Some(ref d) = last_day
                    && !time_buf.is_empty()
                {
                    showtimes_vec.push(format!("{} ore {}", d, time_buf.join(", ")));
                }
                if showtimes_vec.is_empty() {
                    None
                } else {
                    Some(showtimes_vec)
                }
            };

            films.push(Film {
                title,
                url: url.clone(),
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
        "docs/feeds/padova.xml".to_string()
    }
}
