use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

const PROGRAMME_URL: &str = "https://www.lacappellaunderground.org/ariston/programma/";

/// Returns the canonical key for deduplication: strips the _YYYYMMDDHHMM or -YYYYMMDDHHMM
/// suffix from film URLs so we keep only one link per film.
fn canonical_film_key(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if let Some(last_slash) = url.rfind('/') {
        let segment = &url[last_slash + 1..];
        if segment.len() > 13 {
            let (prefix, suffix) = segment.split_at(segment.len() - 12);
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                if let Some(last) = prefix.chars().last() {
                    if last == '_' || last == '-' {
                        let base = &prefix[..prefix.len() - 1];
                        return format!("{}/{}/", &url[..=last_slash], base);
                    }
                }
            }
        }
    }
    format!("{}/", url)
}

/// Scraper for Cinema Ariston (La Cappella Underground) in Trieste.
pub struct CinemaTriesteScraper;

impl CinemaTriesteScraper {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CinemaScraper for CinemaTriesteScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(PROGRAMME_URL)
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
            let link_selector = Selector::parse("a[href*=\"/film/\"]")?;
            let mut urls = Vec::new();
            let mut seen = HashSet::new();

            for a in document.select(&link_selector) {
                let href = match a.value().attr("href") {
                    Some(h) => h.trim(),
                    None => continue,
                };
                if href.is_empty() {
                    continue;
                }
                let full_url = if href.starts_with("http") {
                    href.to_string()
                } else if href.starts_with('/') {
                    format!("https://www.lacappellaunderground.org{}", href)
                } else {
                    format!("https://www.lacappellaunderground.org/{}", href)
                };
                let key = canonical_film_key(&full_url);
                if seen.insert(key) {
                    urls.push(full_url);
                }
            }
            urls
        };

        const BASE: &str = "https://www.lacappellaunderground.org";
        const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

        let mut films = Vec::new();

        for url in film_urls {
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

            let content = match doc
                .select(&Selector::parse("#portfolio-single-content")?)
                .next()
            {
                Some(el) => el,
                None => continue,
            };

            // Title: h1
            let title = content
                .select(&Selector::parse("h1")?)
                .next()
                .map(|h1| {
                    h1.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| url.clone());

            // Meta line: "Director / Country, Year, Duration′ / language" e.g.
            // "Simon Curtis / Gran Bretagna, USA, 2025, 123′ / versione originale..."
            let mut release_date: Option<String> = None;
            let mut running_time: Option<u32> = None;
            let all_text: Vec<String> = content.text().map(|t| t.trim().to_string()).collect();
            for s in &all_text {
                if s.contains('/') && (s.contains("′") || s.contains('\'')) {
                    if let Some(year) = s.split(',').find_map(|p| {
                        let p = p.trim();
                        if p.len() == 4 && p.chars().all(|c| c.is_ascii_digit()) {
                            p.parse::<u32>().ok()
                        } else {
                            None
                        }
                    }) {
                        release_date = Some(year.to_string());
                    }
                    if let Some(minutes) =
                        s.split(|c: char| c == '′' || c == '\'')
                            .next()
                            .and_then(|p| {
                                p.split_whitespace()
                                    .last()
                                    .and_then(|n| n.trim_matches(',').parse::<u32>().ok())
                            })
                    {
                        running_time = Some(minutes);
                    }
                    break;
                }
            }

            // Cast: "con X, Y" - look for text starting with "con "
            let cast = all_text
                .iter()
                .find(|s| s.starts_with("con ") && s.len() > 4)
                .map(|s| s[4..].trim().to_string());

            // Poster: first img with poster in portfolio-single-content
            let poster_url = content
                .select(&Selector::parse("img[src*=\"wp-content/uploads\"]")?)
                .next()
                .and_then(|img| img.value().attr("src"))
                .map(|src| {
                    if src.starts_with("http") {
                        src.to_string()
                    } else {
                        format!("{}{}", BASE, src)
                    }
                });

            // Showtimes: from elementor spans (elementor-icon-list-text, elementor-post-info__item)
            // e.g. <span class="elementor-icon-list-text elementor-post-info__item">Venerdì 13 febbraio</span>
            //      <span class="elementor-icon-list-text elementor-post-info__item">17.30</span>
            // Structure: date, time, v.o., Ingresso (repeated per showtime). Each showtime may be in its own ul.
            // Scan ALL spans in document order. Skip spans inside <a> (In programmazione links) and stop at section headers.
            let mut showtimes = Vec::new();
            let span_selector = Selector::parse(
                "span.elementor-icon-list-text.elementor-post-info__item, span.elementor-post-info__item--type-custom, li.elementor-icon-list-item span",
            )?;
            let mut current_date = String::new();
            for span in content.select(&span_selector) {
                let inside_link = {
                    let mut cur = Some(span);
                    let mut skip = false;
                    for _ in 0..20 {
                        cur = match cur.and_then(|el| el.parent().and_then(ElementRef::wrap)) {
                            Some(p) => {
                                if p.value().name() == "a" {
                                    skip = true;
                                    break;
                                }
                                Some(p)
                            }
                            None => break,
                        };
                    }
                    skip
                };
                if inside_link {
                    continue;
                }
                let text = span
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if text.is_empty() {
                    continue;
                }
                if text == "Rassegne" || text == "In programmazione" {
                    break;
                }
                if text.starts_with("v.") || text.starts_with("Ingresso") {
                    continue;
                }
                if text.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') {
                    let time = text.replace('.', ":");
                    if !current_date.is_empty() {
                        let formatted = format!("{} ore {}", current_date, time);
                        if !showtimes.contains(&formatted) {
                            showtimes.push(formatted);
                        }
                    }
                } else if text.chars().any(|c| c.is_ascii_digit())
                    && (text.contains("braio")   // febbraio
                        || text.contains("enna")  // gennaio
                        || text.contains("arzo")  // marzo
                        || text.contains("rile")  // aprile
                        || text.contains("aggio") // maggio
                        || text.contains("ugno")  // giugno
                        || text.contains("uglio") // luglio
                        || text.contains("osto")  // agosto
                        || text.contains("embre") // settembre, novembre, dicembre
                        || text.contains("obre")) // ottobre
                {
                    current_date = text;
                }
            }

            // Synopsis: paragraphs before "Rassegne" or "In programmazione".
            // Skip but do NOT break on "Ingresso riservato" - synopsis often comes after it.
            // First try p elements; if none found, fall back to div.elementor-widget-text-editor
            // (some pages like Via Convento put synopsis in divs).
            let mut synopsis_parts = Vec::new();
            for selector in ["p", "div.elementor-widget-text-editor"] {
                if !synopsis_parts.is_empty() {
                    break;
                }
                let block_sel = Selector::parse(selector)?;
                for el in content.select(&block_sel) {
                    let text = el
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if text.is_empty() || text.len() <= 30 {
                        continue;
                    }
                    if text == "Rassegne" || text == "In programmazione" {
                        break;
                    }
                    if text.starts_with("Ingresso riservato")
                        || text.starts_with("Ingressi:")
                        || text.starts_with("AA.VV.")
                    {
                        continue;
                    }
                    if !text.starts_with("con ")
                        && !text.contains("versione originale")
                        && !text.contains('′')
                        && !synopsis_parts.contains(&text)
                    {
                        synopsis_parts.push(text);
                    }
                }
            }
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
                url: url.clone(),
                poster_url,
                cast,
                release_date,
                running_time,
                synopsis,
                showtimes,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "feed_trieste.xml".to_string()
    }
}
