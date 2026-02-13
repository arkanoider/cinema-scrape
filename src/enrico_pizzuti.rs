use crate::{CinemaScraper, Film};
use reqwest::{Client, header};
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

/// Scraper for Circolo Cinematografico Enrico Pizzuti (Cinema Turroni Oderzo)
/// Example page: https://www.enricopizzuti.it/
pub struct EnricoPizzutiScraper {
    url: String,
}

impl EnricoPizzutiScraper {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait::async_trait]
impl CinemaScraper for EnricoPizzutiScraper {
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

        // Scope HTML parsing and Cineforum extraction so that non-Send types
        // (`Html`, `ElementRef`, etc.) are dropped before we perform any further awaits.
        let film_urls: Vec<String> = {
            let document = Html::parse_document(&body);

            // Find the Cineforum section (e.g. "<h5>Cineforum 2026</h5>") and, from there,
            // extract the list of film links that belong to that section only.
            let cineforum_h5_selector = Selector::parse("h5")?;
            let mut film_urls: Vec<String> = Vec::new();
            let mut seen_urls: HashSet<String> = HashSet::new();

            // Helper selector used when trying candidate containers.
            let link_selector = Selector::parse("a[href]")?;

            for h5 in document.select(&cineforum_h5_selector) {
                let text = h5
                    .text()
                    .map(|t| t.trim().to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");

                if !text.contains("cineforum") {
                    continue;
                }

                // Walk up a few levels to find a container whose subtree holds film links.
                let mut current = Some(h5);
                for _ in 0..6 {
                    if let Some(cur) = current {
                        let parent = match cur.parent().and_then(ElementRef::wrap) {
                            Some(p) => p,
                            None => break,
                        };

                        let mut urls_in_container = Vec::new();
                        for link in parent.select(&link_selector) {
                            if let Some(href) = link.value().attr("href")
                                && href.contains("/film/")
                            {
                                let full_url = if href.starts_with("http") {
                                    href.to_string()
                                } else {
                                    format!("https://www.enricopizzuti.it{}", href)
                                };
                                if seen_urls.insert(full_url.clone()) {
                                    urls_in_container.push(full_url);
                                }
                            }
                        }

                        if !urls_in_container.is_empty() {
                            film_urls.extend(urls_in_container);
                            break;
                        }

                        current = parent.parent().and_then(ElementRef::wrap);
                    } else {
                        break;
                    }
                }

                // If we already found a suitable container, no need to check further h5s.
                if !film_urls.is_empty() {
                    break;
                }
            }

            film_urls
        };

        // If no Cineforum section was found, return an empty list gracefully.
        if film_urls.is_empty() {
            return Ok(Vec::new());
        }

        // For each film URL in the Cineforum section, open the detail page and extract data
        // from ".container.film-description" and ".film-content".
        let film_container_selector = Selector::parse("div.container.film-description")?;
        let film_date_selector = Selector::parse("div.film-date")?;
        let film_cast_block_selector = Selector::parse("div.film-cast")?;
        let director_selector = Selector::parse("div.director")?;
        let nation_selector = Selector::parse("div.nazione")?;
        let cast_selector = Selector::parse("div.cast")?;
        let h1_selector = Selector::parse("h1")?;
        // Synopsis and poster inside the film-content block
        let film_content_selector = Selector::parse("div.film-content")?;
        let film_text_selector = Selector::parse("div.film-text p")?;
        let film_screens_img_selector = Selector::parse("div.film-screens img")?;

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

            // Find the main film description container.
            let container = match doc.select(&film_container_selector).next() {
                Some(c) => c,
                None => {
                    // If the structure is not as expected, skip this film.
                    continue;
                }
            };

            // Title
            let title = container
                .select(&h1_selector)
                .next()
                .map(|h1| {
                    h1.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Senza titolo".to_string());

            // Date / showtime
            let date_text = container
                .select(&film_date_selector)
                .next()
                .map(|d| {
                    d.text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.is_empty());

            // Cast-related info: director, nation/year, full cast
            let mut cast_parts: Vec<String> = Vec::new();

            if let Some(cast_block) = container.select(&film_cast_block_selector).next() {
                if let Some(dir_el) = cast_block.select(&director_selector).next() {
                    let dir_text = dir_el
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !dir_text.is_empty() {
                        cast_parts.push(dir_text);
                    }
                }

                if let Some(nation_el) = cast_block.select(&nation_selector).next() {
                    let nation_text = nation_el
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !nation_text.is_empty() {
                        cast_parts.push(nation_text);
                    }
                }

                if let Some(cast_el) = cast_block.select(&cast_selector).next() {
                    let cast_text = cast_el
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !cast_text.is_empty() {
                        cast_parts.push(cast_text);
                    }
                }
            }

            let cast = if cast_parts.is_empty() {
                None
            } else {
                Some(cast_parts.join(" | "))
            };

            let showtimes = date_text.clone().map(|d| vec![d.clone()]);

            // Synopsis and poster from film-content section
            let mut synopsis: Option<String> = None;
            let mut poster_url: Option<String> = None;

            if let Some(film_content) = doc.select(&film_content_selector).next() {
                if let Some(text_el) = film_content.select(&film_text_selector).next() {
                    let text = text_el
                        .text()
                        .map(|t| t.trim())
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.is_empty() {
                        synopsis = Some(text);
                    }
                }

                if let Some(img_el) = film_content.select(&film_screens_img_selector).next()
                    && let Some(src) = img_el.value().attr("src")
                    && !src.trim().is_empty()
                {
                    poster_url = Some(src.to_string());
                }
            }

            films.push(Film {
                title,
                url,
                poster_url,
                cast,
                release_date: date_text,
                running_time: None,
                synopsis,
                showtimes,
            });
        }

        Ok(films)
    }

    fn rss_filename(&self) -> String {
        "docs/feeds/enrico_pizzuti.xml".to_string()
    }
}
