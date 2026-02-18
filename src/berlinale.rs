//! Scraper for Berlinale (Berlin International Film Festival).
//! Listing: https://www.berlinale.de/en/programme/on-sale-from-today.html
//! Film page: https://www.berlinale.de/en/2026/programme/202608333.html
//! Film pages embed JSON in a script (initial_result) with title, synopsis, cast, events, etc.

use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Extract the JSON object after "initial_result:" in the page (balanced braces).
fn extract_initial_result_json(html: &str) -> Option<serde_json::Value> {
    let needle = "initial_result:";
    let start = html.find(needle)?;
    let after = &html[start + needle.len()..];
    let obj_start = after.find('{')?;
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escape = false;
    let mut quote = 0u8;
    let bytes = &after.as_bytes()[obj_start..];
    let mut end = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if b == b'\\' {
                escape = true;
            } else if b == quote {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' | b'\'' => {
                in_string = true;
                quote = b;
            }
            b'{' => depth += 1,
            b'}' => {
                if depth == 1 {
                    end = i + 1;
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if end == 0 {
        return None;
    }
    let json_str = &after[obj_start..obj_start + end];
    serde_json::from_str(json_str).ok()
}

const BASE: &str = "https://www.berlinale.de";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// Returns true only for film detail URLs: /en/YEAR/programme/NUMERIC_ID.html (e.g. 202608333).
fn is_film_detail_url(url: &str) -> bool {
    if !url.contains("/programme/") || !url.ends_with(".html") {
        return false;
    }
    let before_html = url.trim_end_matches(".html").trim_end_matches('?');
    let id_part = before_html.rsplit('/').next().unwrap_or("");
    id_part.len() >= 6 && id_part.chars().all(|c| c.is_ascii_digit())
}

/// Extract film detail page URLs from listing. Pattern: /en/YEAR/programme/ID.html (ID = digits only).
fn extract_film_urls(html: &str, base_listing_url: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let link_sel = match Selector::parse("a[href*=\"/programme/\"][href$=\".html\"]") {
        Ok(s) => s,
        Err(_) => return extract_film_urls_from_raw(html, base_listing_url),
    };
    let mut urls = HashSet::new();
    for a in document.select(&link_sel) {
        let href = match a.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        let full = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with('/') {
            format!("{}{}", BASE, href)
        } else {
            format!("{}/{}", BASE, href)
        };
        if is_film_detail_url(&full) {
            urls.insert(full);
        }
    }
    if urls.is_empty() {
        return extract_film_urls_from_raw(html, base_listing_url);
    }
    let mut v: Vec<String> = urls.into_iter().collect();
    v.sort();
    v
}

/// Fallback: find film IDs in raw HTML. Tries "/programme/ID.html" (or "\/programme\/") then "2026" + 5 digits.
fn extract_film_urls_from_raw(html: &str, _base: &str) -> Vec<String> {
    let mut ids = HashSet::new();
    for needle in ["/programme/", "\\/programme\\/"] {
        for (i, _) in html.match_indices(needle) {
            let after = &html[i + needle.len()..];
            let end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
            if end >= 6 {
                let id = &after[..end];
                let rest = after.get(end..).unwrap_or("");
                if rest.starts_with(".html") || rest.starts_with("\\") {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    if ids.is_empty() {
        for (i, _) in html.match_indices("2026") {
            let after = &html[i + 4..];
            if after.len() >= 5 && after[..5].chars().all(|c| c.is_ascii_digit()) {
                let id = format!("2026{}", &after[..5]);
                ids.insert(id);
            }
        }
    }
    let mut v: Vec<String> = ids
        .into_iter()
        .map(|id| format!("{}/en/2026/programme/{}.html", BASE, id))
        .collect();
    v.sort();
    v
}

/// Scraper for Berlinale programme (films on sale / in programme).
pub struct BerlinaleScraper {
    listing_url: String,
}

impl BerlinaleScraper {
    pub fn new(listing_url: String) -> Self {
        Self { listing_url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for BerlinaleScraper {
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>> {
        let resp = client
            .get(self.listing_url.as_str())
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let body = resp.text().await?;

        let film_urls = extract_film_urls(&body, &self.listing_url);
        if film_urls.is_empty() {
            return Ok(Vec::new());
        }

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
            let json = extract_initial_result_json(&body);

            let title = json
                .as_ref()
                .and_then(|j| j.get("title"))
                .and_then(|t| t.as_str())
                .map(String::from)
                .or_else(|| {
                    Selector::parse("meta[property=\"og:title\"]")
                        .ok()
                        .and_then(|sel| {
                            doc.select(&sel)
                                .next()
                                .and_then(|m| m.value().attr("content").map(String::from))
                        })
                        .or_else(|| {
                            Selector::parse("h1").ok().and_then(|sel| {
                                doc.select(&sel).next().map(|h| {
                                    h.text()
                                        .map(|t| t.trim())
                                        .filter(|t| !t.is_empty())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                            })
                        })
                })
                .map(|t| {
                    t.trim_end_matches(" | Berlinale")
                        .trim_end_matches(" – Berlinale")
                        .to_string()
                })
                .and_then(|t| if t.is_empty() { None } else { Some(t) })
                .unwrap_or_default();
            if title.is_empty() || title.starts_with("https://") {
                continue;
            }

            let poster_url = json
                .as_ref()
                .and_then(|j| j.get("filmstills"))
                .and_then(|a| a.as_array())
                .and_then(|arr| {
                    arr.iter().find_map(|s| {
                        let uri = s.get("media")?.get("defaultImage")?.get("uri")?.as_str()?;
                        if uri.contains("plakate") || uri.contains("poster") {
                            Some(if uri.starts_with("http") {
                                uri.to_string()
                            } else {
                                format!("{}{}", BASE, uri)
                            })
                        } else {
                            None
                        }
                    })
                })
                .or_else(|| {
                    json.as_ref()
                        .and_then(|j| j.get("image"))
                        .and_then(|i| i.get("default"))
                        .and_then(|d| d.get("uri"))
                        .and_then(|u| u.as_str())
                        .map(|s| {
                            if s.starts_with("http") {
                                s.to_string()
                            } else {
                                format!("{}{}", BASE, s)
                            }
                        })
                })
                .or_else(|| {
                    Selector::parse("meta[property=\"og:image\"]")
                        .ok()
                        .and_then(|sel| {
                            doc.select(&sel).next().and_then(|m| {
                                m.value().attr("content").map(|s| {
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
                        })
                })
                .or_else(|| {
                    Selector::parse("img[src*=\"berlinale\"], img[src*=\"programme\"]")
                        .ok()
                        .and_then(|sel| {
                            doc.select(&sel).find_map(|img| {
                                img.value().attr("src").map(|s| {
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
                        })
                });

            let (
                mut running_time,
                mut cast,
                mut synopsis_parts,
                mut showtimes,
                mut director_for_title,
            ) = if let Some(ref j) = json {
                let rt = j
                    .get("meta")
                    .and_then(|m| m.as_array())
                    .and_then(|a| a.first())
                    .and_then(|s| s.as_str())
                    .and_then(|s| s.trim_end_matches('\'').trim().parse::<u32>().ok())
                    .or_else(|| {
                        j.get("events")
                            .and_then(|e| e.as_array())
                            .and_then(|a| a.first())
                            .and_then(|e| e.get("time"))
                            .and_then(|t| t.get("durationInMinutes"))
                            .and_then(|d| d.as_u64())
                            .map(|n| n as u32)
                    });
                let by_crew = j
                    .get("crewMembers")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| {
                        let parts: Vec<String> = arr
                            .iter()
                            .filter_map(|m| {
                                let func = m.get("function")?.as_str()?;
                                if func != "Director"
                                    && func != "Screenplay"
                                    && !func.eq_ignore_ascii_case("Screenplay based on")
                                {
                                    return None;
                                }
                                let name =
                                    m.get("names")?.as_array()?.first()?.get("name")?.as_str()?;
                                Some(format!("{} ({})", name, func))
                            })
                            .collect();
                        if parts.is_empty() {
                            None
                        } else {
                            Some("by ".to_string() + &parts.join(", "))
                        }
                    });
                let cast_names = j.get("castMembers").and_then(|c| c.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                let director_for_title = j
                    .get("crewMembers")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| {
                        arr.iter().find(|m| {
                            m.get("function").and_then(|f| f.as_str()) == Some("Director")
                        })
                    })
                    .and_then(|m| {
                        m.get("names")?
                            .as_array()?
                            .first()?
                            .get("name")?
                            .as_str()
                            .map(String::from)
                    })
                    .or_else(|| {
                        j.get("reducedCrewMembers")
                            .and_then(|r| r.as_array())
                            .and_then(|arr| {
                                arr.iter().find_map(|m| {
                                    m.get("name").and_then(|n| n.as_str()).and_then(|s| {
                                        s.strip_suffix(" (Director)").map(String::from)
                                    })
                                })
                            })
                    });
                let cast_str = by_crew
                    .or_else(|| {
                        j.get("reducedCrewMembers")
                            .and_then(|r| r.as_array())
                            .map(|arr| {
                                "by ".to_string()
                                    + &arr
                                        .iter()
                                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                            })
                    })
                    .map(|by_line| {
                        if let Some(ref cn) = cast_names {
                            if cn.is_empty() {
                                by_line
                            } else {
                                format!("{} Cast: {}", by_line, cn)
                            }
                        } else {
                            by_line
                        }
                    });
                let syn = j
                    .get("synopsis")
                    .and_then(|s| s.as_str())
                    .map(|s| {
                        s.replace("<br />", "\n")
                            .replace("<br/>", "\n")
                            .trim()
                            .to_string()
                    })
                    .unwrap_or_default();
                let syn_vec = if syn.is_empty() {
                    Vec::new()
                } else {
                    vec![syn]
                };
                let events: Vec<String> = j
                    .get("events")
                    .and_then(|e| e.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|e| {
                                let date = e
                                    .get("displayDate")
                                    .and_then(|d| d.get("dayAndMonth"))
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("");
                                let weekday = e
                                    .get("displayDate")
                                    .and_then(|d| d.get("weekday"))
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("");
                                let time = e
                                    .get("time")
                                    .and_then(|t| t.get("text"))
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("");
                                let venue =
                                    e.get("venueHall").and_then(|s| s.as_str()).unwrap_or("");
                                if date.is_empty() && time.is_empty() {
                                    None
                                } else {
                                    Some(format!("{} {} {} - {}", weekday, date, time, venue))
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (rt, cast_str, syn_vec, events, director_for_title)
            } else {
                (None, None, Vec::new(), Vec::new(), None)
            };

            if synopsis_parts.is_empty() || cast.is_none() || showtimes.is_empty() {
                let all_text: Vec<String> = doc
                    .root_element()
                    .text()
                    .map(|t| t.trim())
                    .filter(|t| !t.is_empty())
                    .map(String::from)
                    .collect();
                for (i, line) in all_text.iter().enumerate() {
                    if running_time.is_none() && (line.contains(" min") || line == "min") {
                        let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if !num.is_empty() {
                            running_time = num.parse::<u32>().ok();
                        }
                    }
                    if cast.is_none()
                        && (line.eq_ignore_ascii_case("Director:")
                            || line.eq_ignore_ascii_case("Regie:"))
                        && let Some(next) = all_text.get(i + 1)
                    {
                        cast = Some(next.clone());
                        director_for_title = Some(next.clone());
                    }
                    if cast.is_some()
                        && line.eq_ignore_ascii_case("Cast:")
                        && let Some(next) = all_text.get(i + 1)
                    {
                        let existing = cast.take().unwrap_or_default();
                        cast = Some(if existing.is_empty() {
                            next.clone()
                        } else {
                            format!("{}. {}", existing, next)
                        });
                    }
                    if synopsis_parts.is_empty()
                        && (line.eq_ignore_ascii_case("Synopsis")
                            || line.eq_ignore_ascii_case("Plot"))
                    {
                        for s in all_text.iter().skip(i + 1).take(14) {
                            if s.len() > 50
                                && !s.starts_with("http")
                                && !s.eq_ignore_ascii_case("Director:")
                                && !s.eq_ignore_ascii_case("Cast:")
                            {
                                synopsis_parts.push(s.clone());
                            } else if s.len() < 10 {
                                break;
                            }
                        }
                    }
                    if showtimes.is_empty()
                        && (line.contains("Screenings")
                            || line.contains("Februar")
                            || line.contains("February"))
                        && (line.contains(':') || line.chars().any(|c| c.is_ascii_digit()))
                    {
                        showtimes.push(line.clone());
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

            let display_title = director_for_title
                .as_ref()
                .map(|d| format!("{} by {}", title.trim(), d))
                .unwrap_or_else(|| title.clone());
            films.push(Film {
                title: display_title,
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
        "docs/feeds/berlinale.xml".to_string()
    }
}
