//! Scraper for Vista Theater Hollywood.
//! Schedule: https://www.vistatheaterhollywood.com/ (#now-playing)

use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{ElementRef, Html, Selector};

const HOME_URL: &str = "https://www.vistatheaterhollywood.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

pub struct VistaTheatreScraper {
    url: String,
}

impl VistaTheatreScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for VistaTheatreScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(&self.url)
            .header(header::USER_AGENT, USER_AGENT)
            .header(header::ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;
        Ok(parse_homepage(&body))
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/vista_theatre.xml".to_string()
    }
}

fn parse_homepage(html: &str) -> Vec<Film> {
    let doc = Html::parse_document(html);
    let row_sel = match Selector::parse("#now-playing .shows__grid > .shows__grid--row") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let times_sel = match Selector::parse(".shows__grid--cell:first-child") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let content_sel = match Selector::parse(".shows__grid--cell:nth-child(2)") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut films = Vec::new();
    for row in doc.select(&row_sel) {
        let showtimes = row
            .select(&times_sel)
            .next()
            .map(parse_showtimes)
            .unwrap_or_default();
        let ticket_url = row
            .select(&times_sel)
            .next()
            .and_then(first_ticket_url);
        let Some(content_cell) = row.select(&content_sel).next() else {
            continue;
        };

        let panel_sel = Selector::parse(".shows__feature-panel").ok();
        let panels: Vec<ElementRef<'_>> = panel_sel
            .as_ref()
            .map(|sel| content_cell.select(sel).collect())
            .unwrap_or_default();

        if panels.is_empty() {
            if let Some(ref sel) = Selector::parse(".content").ok()
                && let Some(content) = content_cell.select(sel).next()
                && let Some(film) = parse_film_block(
                    content,
                    None,
                    &showtimes,
                    0,
                    content_cell,
                    ticket_url.as_deref(),
                )
            {
                films.push(film);
            }
        } else {
            for (idx, panel) in panels.iter().enumerate() {
                let poster = poster_for_panel(content_cell, idx);
                if let Some(film) = parse_film_block(
                    *panel,
                    poster,
                    &showtimes,
                    idx,
                    content_cell,
                    ticket_url.as_deref(),
                ) {
                    films.push(film);
                }
            }
        }
    }
    films
}

fn parse_showtimes(times_cell: ElementRef<'_>) -> Vec<String> {
    let time_link_sel = match Selector::parse("a.card__button") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let walk_sel = match Selector::parse(".inner p, .inner div.times") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut month = String::new();
    let mut day_num = String::new();
    let mut weekday = String::new();
    let mut showtimes = Vec::new();

    for child in times_cell.select(&walk_sel) {
        let class = child.value().attr("class").unwrap_or("");
        let tag = child.value().name();

        if class.contains("month") {
            month = child_text(&child);
        } else if class.contains("text__size-2") {
            day_num = child_text(&child);
        } else if tag == "p" && !class.contains("month") && !class.contains("text__size-2") {
            weekday = child_text(&child);
        } else if tag == "div" && class.contains("times") {
            for a in child.select(&time_link_sel) {
                let time = child_text(&a);
                if time.is_empty() {
                    continue;
                }
                let date_part = format!("{} {} {}", month, day_num, weekday)
                    .trim()
                    .to_string();
                showtimes.push(format!("{} ore {}", date_part, time));
            }
        }
    }
    showtimes
}

fn first_ticket_url(times_cell: ElementRef<'_>) -> Option<String> {
    Selector::parse("a.card__button")
        .ok()
        .and_then(|sel| {
            times_cell
                .select(&sel)
                .next()
                .and_then(|a| a.value().attr("href"))
                .map(|s| s.to_string())
        })
}

fn poster_for_panel(content_cell: ElementRef<'_>, panel_index: usize) -> Option<String> {
    let img_sel = Selector::parse(".shows__grid--poster img").ok()?;
    for img in content_cell.select(&img_sel) {
        let idx = img
            .value()
            .attr("data-feature-index")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        if idx == panel_index {
            return img.value().attr("src").map(|s| s.trim().to_string());
        }
    }
    content_cell
        .select(&img_sel)
        .next()
        .and_then(|img| img.value().attr("src"))
        .map(|s| s.trim().to_string())
}

fn parse_film_block(
    block: ElementRef<'_>,
    poster_url: Option<String>,
    showtimes: &[String],
    panel_index: usize,
    content_cell: ElementRef<'_>,
    ticket_url: Option<&str>,
) -> Option<Film> {
    let title_sel = Selector::parse("h3.alt, h4.alt").ok()?;
    let title = block
        .select(&title_sel)
        .next()
        .map(|h| child_text(&h))
        .filter(|t| !t.is_empty())?;

    let poster_url = poster_url.or_else(|| {
        if panel_index == 0 {
            Selector::parse(".shows__grid--poster-image.active")
                .ok()
                .and_then(|sel| {
                    content_cell
                        .select(&sel)
                        .next()
                        .and_then(|img| img.value().attr("src"))
                        .map(|s| s.trim().to_string())
                })
        } else {
            poster_for_panel(content_cell, panel_index)
        }
    });

    let mut year = None;
    let mut running_time = None;
    let mut director = None;
    let mut writers = None;
    let mut stars = None;
    let mut synopsis_parts = Vec::new();

    let summary_sel = Selector::parse(".summary p").ok()?;

    for p in block.select(&summary_sel) {
        let text = child_text(&p);
        if text.len() > 20 {
            synopsis_parts.push(text);
        }
    }

    let p_sel = Selector::parse("p").ok()?;
    let ps: Vec<ElementRef<'_>> = block.select(&p_sel).collect();
    let mut i = 0;
    while i < ps.len() {
        let class = ps[i].value().attr("class").unwrap_or("");
        if class.contains("text__size-4") {
            let label = child_text(&ps[i]);
            if i + 1 < ps.len() {
                let next_class = ps[i + 1].value().attr("class").unwrap_or("");
                if !next_class.contains("text__size-4") {
                    let value = child_text(&ps[i + 1]);
                    if label.eq_ignore_ascii_case("Director") && !value.is_empty() {
                        director = Some(value);
                    } else if label.eq_ignore_ascii_case("Writers") && !value.is_empty() {
                        writers = Some(value);
                    } else if label.eq_ignore_ascii_case("Stars") && !value.is_empty() {
                        stars = Some(value);
                    }
                    i += 2;
                    continue;
                }
            }
        } else {
            let text = child_text(&ps[i]);
            if text.contains('|') && text.contains('h') && text.contains('m') {
                let (y, mins) = parse_meta_line(&text);
                year = y;
                running_time = mins;
            }
        }
        i += 1;
    }

    let mut cast_parts = Vec::new();
    if let Some(ref d) = director.filter(|s| !s.is_empty()) {
        cast_parts.push(format!("Director: {}", d));
    }
    if let Some(ref w) = writers.filter(|s| !s.is_empty()) {
        cast_parts.push(format!("Writers: {}", w));
    }
    if let Some(ref s) = stars.filter(|s| !s.is_empty()) {
        cast_parts.push(format!("Stars: {}", s));
    }
    let cast = if cast_parts.is_empty() {
        None
    } else {
        Some(cast_parts.join(" | "))
    };

    let synopsis = if synopsis_parts.is_empty() {
        None
    } else {
        Some(synopsis_parts.join("\n\n"))
    };

    let url = ticket_url
        .map(String::from)
        .unwrap_or_else(|| HOME_URL.to_string());

    Some(Film {
        title,
        url,
        poster_url,
        cast,
        release_date: year,
        running_time,
        synopsis,
        showtimes: if showtimes.is_empty() {
            None
        } else {
            Some(showtimes.to_vec())
        },
    })
}

fn parse_meta_line(line: &str) -> (Option<String>, Option<u32>) {
    let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
    let year = parts.first().and_then(|p| {
        p.chars()
            .take(4)
            .collect::<String>()
            .parse::<u32>()
            .ok()
            .map(|y| y.to_string())
    });
    let running_time = parts.get(1).and_then(|p| parse_duration(p));
    (year, running_time)
}

fn parse_duration(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase();
    let mut hours = 0u32;
    let mut mins = 0u32;
    if let Some(h_pos) = s.find('h') {
        hours = s[..h_pos].trim().parse().ok()?;
        let rest = &s[h_pos + 1..];
        if let Some(m_pos) = rest.find('m') {
            mins = rest[..m_pos].trim().parse().ok()?;
        }
    } else if let Some(m_pos) = s.find('m') {
        mins = s[..m_pos].trim().parse().ok()?;
    }
    Some(hours * 60 + mins)
}

fn child_text(el: &ElementRef<'_>) -> String {
    el.text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_works() {
        assert_eq!(parse_duration("1h 35m"), Some(95));
        assert_eq!(parse_duration("2h 17m"), Some(137));
    }

    #[test]
    fn parse_sample_row() {
        let html = r#"<!DOCTYPE html><html><body>
        <section id="now-playing"><div class="shows__grid">
        <div class="shows__grid--row">
          <div class="shows__grid--cell">
            <div class="inner">
              <p class="text__size-4 month">June</p>
              <p class="text__size-2">6th</p><p>Saturday</p>
              <div class="times"><div class="group"><a class="card__button btn --red">10:00 am</a></div></div>
            </div>
          </div>
          <div class="shows__grid--cell">
            <div class="shows__grid--poster"><img src="https://example.com/poster.jpg" class="shows__grid--poster-image active"></div>
            <div class="content">
              <h3 class="alt">L.A. Story</h3>
              <div>
                <p>1991 | 1h 35m | 35mm Presentation</p>
                <div class="summary"><p>With the help of a talking freeway billboard...</p></div>
                <p class="text__size-4">Director</p><p>Mick Jackson</p>
                <p class="text__size-4">Writers</p><p>Steve Martin</p>
                <p class="text__size-4">Stars</p><p>Steve Martin, Victoria Tennant</p>
              </div>
            </div>
          </div>
        </div>
        </div></section></body></html>"#;
        let films = parse_homepage(html);
        assert_eq!(films.len(), 1);
        let f = &films[0];
        assert_eq!(f.title, "L.A. Story");
        assert_eq!(f.release_date.as_deref(), Some("1991"));
        assert_eq!(f.running_time, Some(95));
        assert!(f.cast.as_ref().unwrap().contains("Mick Jackson"));
        assert!(f.synopsis.as_ref().unwrap().contains("freeway billboard"));
        assert!(f.showtimes.as_ref().unwrap()[0].contains("10:00 am"));
    }
}
