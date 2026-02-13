use crate::{CinemaScraper, Film};
use chrono::{DateTime, Datelike};
use reqwest::{Client, header};
use serde::Deserialize;

static IT_WEEKDAY: [&str; 7] = [
    "lunedì",
    "martedì",
    "mercoledì",
    "giovedì",
    "venerdì",
    "sabato",
    "domenica",
];

fn format_showtime(dt: &DateTime<chrono::Utc>) -> String {
    let wd = dt.weekday().num_days_from_monday() as usize;
    let day_name = IT_WEEKDAY.get(wd).copied().unwrap_or("");
    format!(
        "{} {:02}/{:02} ore {}",
        day_name,
        dt.day(),
        dt.month(),
        dt.format("%H:%M")
    )
}

const JSON_URL: &str = "https://www.cinemarex.it/pages/rexJsonCompact.php";
const PROGRAMMAZIONE_BASE: &str = "https://www.cinemarex.it/programmazione";

/// Scraper for Cinema Rex Padova (uses JSON API; programmazione page is JS-rendered).
pub struct FeedPadovaScraper {
    #[allow(dead_code)]
    url: String,
}

impl FeedPadovaScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[derive(Debug, Deserialize)]
struct RexResponse {
    titoli: Vec<RexTitolo>,
}

#[derive(Debug, Deserialize)]
struct RexTitolo {
    titolo: String,
    #[serde(default)]
    autore: String,
    #[serde(default)]
    durata: String,
    #[serde(default)]
    descrizione: String,
    #[serde(default)]
    #[allow(dead_code)]
    locandina: String,
    #[serde(default)]
    categoria_film: String, // "y" for films, "n" for theater/music/etc
    eventi: Vec<RexEvento>,
}

#[derive(Debug, Deserialize)]
struct RexEvento {
    inizio: i64, // milliseconds since epoch
}

fn event_name_slug(titolo: &str) -> String {
    titolo
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
}

#[async_trait::async_trait]
impl CinemaScraper for FeedPadovaScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(JSON_URL)
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
        let data: RexResponse = serde_json::from_str(&body)?;

        let mut films = Vec::new();
        for t in data.titoli {
            // Only include items categorized as "Film"
            if t.categoria_film != "y" {
                continue;
            }

            let title = t.titolo.trim().to_string();
            if title.is_empty() {
                continue;
            }

            let slug = event_name_slug(&title);
            let url = format!("{}/evento?eventName={}", PROGRAMMAZIONE_BASE, slug);

            let running_time = t.durata.trim().parse::<u32>().ok();

            let synopsis = t.descrizione.trim();
            let synopsis = if synopsis.is_empty() {
                None
            } else {
                Some(synopsis.to_string())
            };

            let cast = if t.autore.trim().is_empty() {
                None
            } else {
                Some(format!("Regia: {}", t.autore.trim()))
            };

            let showtimes: Vec<String> = t
                .eventi
                .iter()
                .filter_map(|e| {
                    DateTime::from_timestamp_millis(e.inizio).map(|dt| format_showtime(&dt))
                })
                .collect();

            // Avoid duplicate date+time (same film can have multiple eventi with same slot)
            let showtimes: Vec<String> = {
                let mut seen = std::collections::HashSet::new();
                let mut out = Vec::new();
                for s in showtimes {
                    if seen.insert(s.clone()) {
                        out.push(s);
                    }
                }
                out
            };

            let showtimes = if showtimes.is_empty() {
                None
            } else {
                Some(showtimes)
            };

            films.push(Film {
                title,
                url,
                poster_url: None,
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
        "feed_padova.xml".to_string()
    }
}
