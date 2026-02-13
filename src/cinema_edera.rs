use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Base URL (origin) derived from a full URL, e.g. "https://www.cinemamanzoni.it"
fn base_from_listing_url(listing_url: &str) -> String {
    let after_proto = listing_url.find("://").map(|i| i + 3).unwrap_or(0);
    let path_start = listing_url[after_proto..]
        .find('/')
        .map(|i| after_proto + i)
        .unwrap_or(listing_url.len());
    listing_url[..path_start].to_string()
}

/// Scraper for cinema sites that share the Edera HTML structure (e.g. Cinema Edera, Cinema Manzoni).
pub struct CinemaEderaScraper {
    url: String,
    base: String,
}

impl CinemaEderaScraper {
    pub fn new(url: String) -> Self {
        let base = base_from_listing_url(&url);
        Self { url, base }
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

        // Parse listing page in a block so document is dropped before any subsequent await
        let mut films = {
            let document = Html::parse_document(&body);
            let table_selector = Selector::parse("#timetable")?;
            let table = document
                .select(&table_selector)
                .next()
                .ok_or("Could not find timetable table")?;
            let row_selector = Selector::parse("tbody tr")?;
            let link_selector = Selector::parse("a.category__item")?;
            let title_selector = Selector::parse("strong")?;
            let mut seen_urls = HashSet::new();
            let mut films = Vec::new();

            for row in table.select(&row_selector) {
                for link in row.select(&link_selector) {
                    let href = link.value().attr("href").unwrap_or("");
                    let full_url = format!("{}{}", self.base, href);
                    if seen_urls.contains(&full_url) {
                        continue;
                    }
                    // Title from <strong> only; orari (dates/times) are in div.time-select on the film page
                    let title = link
                        .select(&title_selector)
                        .next()
                        .map(|e| e.text().collect::<String>().trim().to_string())
                        .unwrap_or_default();
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
                            showtimes: None,
                        });
                    }
                }
            }
            films
        };

        // Fetch each film page to get poster, movie__option info, and synopsis
        let base = &self.base;
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36";

        for film in films.iter_mut() {
            if let Ok(resp) = client
                .get(&film.url)
                .header(header::USER_AGENT, user_agent)
                .send()
                .await
                && let Ok(body) = resp.text().await
            {
                let doc = Html::parse_document(&body);

                // Poster: img inside .movie__images
                if let Ok(img_sel) = Selector::parse(".movie__images img.img-responsive")
                    && let Some(img) = doc.select(&img_sel).next()
                    && let Some(src) = img.value().attr("src")
                {
                    film.poster_url = Some(if src.starts_with("http") {
                        src.to_string()
                    } else {
                        format!("{}{}", base, src)
                    });
                }

                // Running time: p.movie__time e.g. "132 min"
                if let Ok(time_sel) = Selector::parse("p.movie__time")
                    && let Some(p) = doc.select(&time_sel).next()
                {
                    let text = p.text().collect::<String>();
                    if let Some(num) = text
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        film.running_time = Some(num);
                    }
                }

                // All options from div.movie__option: <p><strong>Label</strong>: value</p>
                let mut option_parts = Vec::new();
                if let (Ok(option_sel), Ok(p_sel)) =
                    (Selector::parse("div.movie__option"), Selector::parse("p"))
                    && let Some(option_div) = doc.select(&option_sel).next()
                {
                    for p in option_div.select(&p_sel) {
                        let text = p.text().collect::<String>();
                        let text = text.trim();
                        if let Some((label, value)) = text.split_once(':') {
                            let label = label.trim();
                            let value = value.trim();
                            match label {
                                "Cast" => film.cast = Some(value.to_string()),
                                "Anno" => film.release_date = Some(value.to_string()),
                                _ => option_parts.push(format!("{}: {}", label, value)),
                            }
                        }
                    }
                }

                // Synopsis: p.movie__describe (Trama) + optional extra info from movie__option
                // and long-form description in the main content area (h3 / strong blocks).
                let mut synopsis_parts = Vec::new();
                if !option_parts.is_empty() {
                    synopsis_parts.push(option_parts.join(" | "));
                }
                if let Ok(desc_sel) = Selector::parse("p.movie__describe")
                    && let Some(desc) = doc.select(&desc_sel).next()
                {
                    let trama = desc.text().collect::<String>();
                    let trama = trama.trim();
                    if !trama.is_empty() {
                        synopsis_parts.push(trama.to_string());
                    }
                }
                // Long text description (e.g. "Trama" section) can appear as headings
                // or highlighted text inside the main content wrapper. We append those
                // as well so Edera entries have a rich synopsis similar to the other cinemas.
                if let Ok(h3_sel) = Selector::parse("#main-content-wrapper section h3") {
                    for h3 in doc.select(&h3_sel) {
                        let text = h3.text().collect::<String>();
                        let text = text.trim();
                        if !text.is_empty() {
                            synopsis_parts.push(text.to_string());
                        }
                    }
                }
                // Some pages (e.g. Marty Supreme) put important synopsis lines in <strong>
                // elements rather than plain paragraphs. Collect those too, but skip labels
                // like "Genere", "Paese", etc.
                if let Ok(strong_sel) = Selector::parse("#main-content-wrapper section strong") {
                    for strong in doc.select(&strong_sel) {
                        let text = strong.text().collect::<String>();
                        let text = text.trim();
                        if text.is_empty() {
                            continue;
                        }
                        let lower = text.to_lowercase();
                        if lower.starts_with("genere")
                            || lower.starts_with("paese")
                            || lower.starts_with("regia")
                            || lower.starts_with("cast")
                            || lower.starts_with("anno")
                            || lower.starts_with("lingua")
                            || lower.contains("orari spettacoli")
                        {
                            continue;
                        }
                        synopsis_parts.push(text.to_string());
                    }
                }
                if !synopsis_parts.is_empty() {
                    film.synopsis = Some(synopsis_parts.join("\n\n"));
                }

                // Showtimes from div.time-select: "Lunedì 9 Febbraio ore 17:15", etc.
                let mut showtimes = Vec::new();
                if let (Ok(time_select_sel), Ok(group_sel), Ok(place_sel), Ok(item_sel)) = (
                    Selector::parse("div.time-select"),
                    Selector::parse("div.time-select__group"),
                    Selector::parse("p.time-select__place"),
                    Selector::parse("li.time-select__item"),
                ) && let Some(time_select) = doc.select(&time_select_sel).next()
                {
                    for group in time_select.select(&group_sel) {
                        let date = group
                            .select(&place_sel)
                            .next()
                            .map(|p| p.text().collect::<String>().trim().to_string())
                            .unwrap_or_default();
                        for li in group.select(&item_sel) {
                            let text = li.text().collect::<String>();
                            let time = text
                                .split_whitespace()
                                .find(|s| s.contains(':'))
                                .unwrap_or("")
                                .trim();
                            if !date.is_empty() && !time.is_empty() {
                                showtimes.push(format!("{} ore {}", date, time));
                            }
                        }
                    }
                }
                if !showtimes.is_empty() {
                    film.showtimes = Some(showtimes);
                }
            }
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/cinema_edera.xml".to_string()
    }
}
