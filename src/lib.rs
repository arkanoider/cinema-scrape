use reqwest::Client;
use rss::{ChannelBuilder, ItemBuilder};

/// Common film data structure that all scrapers should produce
#[derive(Debug, Clone)]
pub struct Film {
    pub title: String,
    pub url: String,
    pub poster_url: Option<String>,
    pub cast: Option<String>,
    pub release_date: Option<String>,
    pub running_time: Option<u32>, // in minutes
    pub synopsis: Option<String>,
}

/// Trait that all cinema scrapers must implement
#[async_trait::async_trait]
pub trait CinemaScraper {
    /// Fetch films from the cinema website
    async fn fetch_films(&self, client: &Client) -> Result<Vec<Film>, Box<dyn std::error::Error>>;

    /// Optional: warm-up request to get cookies/auth (default: no-op)
    async fn warm_up(&self, _client: &Client) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Generate RSS feed name for this scraper (used for filename)
    fn rss_filename(&self) -> String;
}

/// Generate RSS feed from a list of films
pub fn generate_rss(
    films: &[Film],
    channel_title: &str,
    channel_link: &str,
    channel_description: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut items = Vec::new();

    for film in films {
        // Build description from available fields
        let mut description_parts = Vec::new();
        
        if let Some(ref synopsis) = film.synopsis {
            description_parts.push(synopsis.clone());
        }
        
        if let Some(ref cast) = film.cast {
            description_parts.push(format!("Cast: {}", cast));
        }
        
        if let Some(ref date) = film.release_date {
            description_parts.push(format!("Data: {}", date));
        }
        
        if let Some(time) = film.running_time {
            description_parts.push(format!("Durata: {} minuti", time));
        }
        
        if let Some(ref poster) = film.poster_url {
            description_parts.push(format!("<img src=\"{}\" alt=\"Poster\" />", poster));
        }

        let description = if description_parts.is_empty() {
            format!("Film: {}", film.title)
        } else {
            description_parts.join("<br/>\n")
        };

        let guid = rss::Guid {
            value: film.url.clone(),
            permalink: true,
        };

        // Try to parse release_date as pubDate if it looks like a date
        let pub_date = if let Some(ref date_str) = film.release_date {
            // Simple heuristic: if it contains date-like patterns, use current time
            // (you could parse Italian dates more precisely if needed)
            if date_str.contains("Febbraio")
                || date_str.contains("Gennaio")
                || date_str.contains("Marzo")
            {
                Some(chrono::Utc::now().to_rfc2822())
            } else {
                None
            }
        } else {
            None
        };

        let mut item_builder = ItemBuilder::default();
        item_builder.title(film.title.clone());
        item_builder.link(film.url.clone());
        item_builder.description(description);
        item_builder.guid(guid);
        if let Some(date) = pub_date {
            item_builder.pub_date(date);
        }

        let item = item_builder.build();
        items.push(item);
    }

    let channel = ChannelBuilder::default()
        .title(channel_title)
        .link(channel_link)
        .description(channel_description)
        .items(items)
        .build();

    let mut buf = Vec::new();
    channel.write_to(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}
