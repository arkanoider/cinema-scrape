//! Scraper for The New Beverly Cinema (Quentin Tarantino's revival theater in LA).
//! Schedule: https://thenewbev.com/schedule/
//! Each program page has synopsis, Director/Writer/Starring/Year/Country/Format/Running time.

use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashMap;

const BASE: &str = "https://thenewbev.com";
const SCHEDULE_URL: &str = "https://thenewbev.com/schedule/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// One screening from the schedule (before merging by URL).
struct ScheduleEntry {
    title: String,
    url: String,
    showtime: String,
    poster_url: Option<String>,
}

/// After deduplication: one per unique program URL, with all showtimes merged.
struct UniqueProgram {
    title: String,
    url: String,
    showtimes: Vec<String>,
    poster_url: Option<String>,
}

pub struct NewBevScraper {
    schedule_url: String,
}

impl NewBevScraper {
    pub fn new() -> Self {
        Self {
            schedule_url: SCHEDULE_URL.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for NewBevScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(&self.schedule_url)
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;

        let entries = parse_schedule(&body)?;
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Deduplicate by program URL: one fetch per unique link, merge showtimes.
        let mut by_url: HashMap<String, UniqueProgram> = HashMap::new();
        for entry in entries {
            by_url
                .entry(entry.url.clone())
                .or_insert_with(|| UniqueProgram {
                    title: entry.title.clone(),
                    url: entry.url.clone(),
                    showtimes: Vec::new(),
                    poster_url: entry.poster_url.clone(),
                })
                .showtimes
                .push(entry.showtime);
        }
        let unique: Vec<UniqueProgram> = by_url.into_values().collect();

        let mut films = Vec::with_capacity(unique.len());
        for program in unique {
            let (synopsis, cast, running_time, poster_from_page) =
                fetch_program_page(client, &program.url).await;

            let poster_url = poster_from_page.or(program.poster_url);
            let cast = if cast.is_empty() { None } else { Some(cast) };
            let synopsis = if synopsis.is_empty() { None } else { Some(synopsis) };

            films.push(Film {
                title: program.title,
                url: program.url,
                poster_url,
                cast,
                release_date: None,
                running_time,
                synopsis,
                showtimes: Some(program.showtimes),
            });
        }
        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/tarantino.xml".to_string()
    }
}

fn parse_schedule(html: &str) -> Result<Vec<ScheduleEntry>, Box<dyn std::error::Error>> {
    let doc = Html::parse_document(html);
    let card_sel = Selector::parse("article.event-card").map_err(|e| e.to_string())?;
    let link_sel = Selector::parse("a[href*='/program/']").map_err(|e| e.to_string())?;
    let title_sel = Selector::parse("h4.event-card__title").map_err(|e| e.to_string())?;
    let time_sel = Selector::parse("time.event-card__time").map_err(|e| e.to_string())?;
    let date_day_sel = Selector::parse("span.event-card__day").map_err(|e| e.to_string())?;
    let date_month_sel = Selector::parse("span.event-card__month").map_err(|e| e.to_string())?;
    let date_numb_sel = Selector::parse("span.event-card__numb").map_err(|e| e.to_string())?;
    let img_sel = Selector::parse("figure.event-card__img img").map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for article in doc.select(&card_sel) {
        let link = match article.select(&link_sel).next() {
            Some(a) => a,
            None => continue,
        };
        let href = link.value().attr("href").unwrap_or("").trim();
        if href.is_empty() || !href.contains("/program/") {
            continue;
        }
        let url = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with('/') {
            format!("{}{}", BASE, href)
        } else {
            format!("{}/{}", BASE, href)
        };
        let url = url.trim_end_matches('/').to_string();
        if !url.contains("thenewbev.com") || !url.contains("/program/") {
            continue;
        }

        let title = link
            .select(&title_sel)
            .next()
            .map(|h| {
                h.text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace("  ", " ")
                    .trim()
                    .to_string()
            })
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let day: String = link
            .select(&date_day_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().replace(',', ""))
            .unwrap_or_default();
        let month: String = link
            .select(&date_month_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let numb: String = link
            .select(&date_numb_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let times: Vec<String> = link
            .select(&time_sel)
            .map(|t| t.text().collect::<String>().trim().to_string())
            .collect();
        let showtime = if times.is_empty() {
            format!("{} {} {}", day, month, numb)
        } else {
            format!("{} {} {} - {}", day, month, numb, times.join(" / "))
        };
        let showtime = showtime.trim().to_string();

        let poster_url = link
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(|s| {
                let s = s.trim();
                if s.starts_with("http") {
                    s.to_string()
                } else if s.starts_with('/') {
                    format!("{}{}", BASE, s)
                } else {
                    format!("{}/{}", BASE, s)
                }
            });

        entries.push(ScheduleEntry {
            title,
            url,
            showtime,
            poster_url,
        });
    }
    Ok(entries)
}

/// Fetch program page and return (synopsis, cast_info, running_time_minutes, poster_url).
async fn fetch_program_page(
    client: &Client,
    url: &str,
) -> (
    String,
    String,
    Option<u32>,
    Option<String>,
) {
    let resp = match client
        .get(url)
        .header(header::USER_AGENT, USER_AGENT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return (String::new(), String::new(), None, None),
    };
    let resp = match resp.error_for_status() {
        Ok(r) => r,
        Err(_) => return (String::new(), String::new(), None, None),
    };
    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => return (String::new(), String::new(), None, None),
    };
    parse_program_page(&body)
}

fn parse_program_page(html: &str) -> (String, String, Option<u32>, Option<String>) {
    let doc = Html::parse_document(html);

    // Poster: og:image first, then first .movie__poster img or .movie-mast__poster-img
    let poster_url = Selector::parse("meta[property=\"og:image\"]")
        .ok()
        .and_then(|sel| {
            doc.select(&sel)
                .next()
                .and_then(|m| m.value().attr("content"))
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Selector::parse(".movie__poster img, .movie-mast__poster-img img")
                .ok()
                .and_then(|sel| {
                    doc.select(&sel)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                        .map(|s| {
                            let s = s.trim();
                            if s.starts_with("http") {
                                s.to_string()
                            } else if s.starts_with('/') {
                                format!("{}{}", BASE, s)
                            } else {
                                format!("{}/{}", BASE, s)
                            }
                        })
                })
        });

    // Director, Writer, Starring, Year, Country, Format, Running Time (site uses one <dl> per label)
    let mut running_time: Option<u32> = None;
    let mut cast_parts: Vec<String> = Vec::new();
    let dt_sel = Selector::parse("dl dt").ok();
    let dd_sel = Selector::parse("dl dd").ok();
    if let (Some(ref dt_sel), Some(ref dd_sel)) = (dt_sel, dd_sel) {
        let dts: Vec<String> = doc
            .select(dt_sel)
            .map(|e| e.text().collect::<String>().trim().to_string())
            .collect();
        let dds: Vec<String> = doc
            .select(dd_sel)
            .map(|e| e.text().collect::<String>().trim().to_string())
            .collect();
        for (i, dt) in dts.iter().enumerate() {
            let dd = dds.get(i).map(String::as_str).unwrap_or("");
            if dt.eq_ignore_ascii_case("Running Time") {
                let mins: String = dd.chars().filter(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = mins.parse::<u32>() {
                    running_time = Some(n);
                }
            } else if !dd.is_empty()
                && (dt.eq_ignore_ascii_case("Director")
                    || dt.eq_ignore_ascii_case("Writer")
                    || dt.eq_ignore_ascii_case("Starring")
                    || dt.eq_ignore_ascii_case("Year")
                    || dt.eq_ignore_ascii_case("Country")
                    || dt.eq_ignore_ascii_case("Format"))
            {
                cast_parts.push(format!("{}: {}", dt, dd));
            }
        }
    }

    // Synopsis: .movie__content p (main program text) then .entry-content p fallback
    let mut synopsis_parts = Vec::new();
    for selector in [
        ".movie__content p",
        "section.movies .movie__content p",
        ".entry-content p",
        ".post-content p",
    ] {
        if let Ok(sel) = Selector::parse(selector) {
            for el in doc.select(&sel) {
                let text: String = el
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = text.trim();
                if text.len() < 50 {
                    continue;
                }
                if text.eq_ignore_ascii_case("Buy Tickets")
                    || text.eq_ignore_ascii_case("View Trailer")
                    || text.contains("ticketing.")
                    || text.contains("veezi.com")
                {
                    continue;
                }
                if text.contains("New Beverly blog") && text.len() < 150 {
                    continue;
                }
                synopsis_parts.push(text.to_string());
                if synopsis_parts.len() >= 8 {
                    break;
                }
            }
            if !synopsis_parts.is_empty() {
                break;
            }
        }
    }

    let synopsis = synopsis_parts.join("\n\n");
    let cast = cast_parts.join(" | ");
    (synopsis, cast, running_time, poster_url)
}
