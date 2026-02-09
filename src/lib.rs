use reqwest::Client;
use rss::{Category, ChannelBuilder, ItemBuilder};

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
    /// Showtimes as "Lunedì 9 Febbraio ore 17:15", "Martedì 10 Febbraio ore 19:10", etc.
    pub showtimes: Option<Vec<String>>,
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

/// Build description and optional pub_date for a film (shared by generate_rss and generate_rss_merged).
fn film_description_and_pub_date(film: &Film) -> (String, Option<String>) {
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
    if let Some(ref showtimes) = film.showtimes {
        if !showtimes.is_empty() {
            description_parts.push(format!("Orari: {}", showtimes.join(", ")));
        }
    }
    let description = if description_parts.is_empty() {
        format!("Film: {}", film.title)
    } else {
        description_parts.join("<br/>\n")
    };
    let pub_date = film.release_date.as_ref().and_then(|date_str| {
        if date_str.contains("Febbraio") || date_str.contains("Gennaio") || date_str.contains("Marzo") {
            Some(chrono::Utc::now().to_rfc2822())
        } else {
            None
        }
    });
    (description, pub_date)
}

/// Generate RSS feed from a list of films (single cinema).
pub fn generate_rss(
    films: &[Film],
    channel_title: &str,
    channel_link: &str,
    channel_description: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for film in films {
        let (description, pub_date) = film_description_and_pub_date(film);
        let guid = rss::Guid {
            value: film.url.clone(),
            permalink: true,
        };
        let mut item_builder = ItemBuilder::default();
        item_builder
            .title(film.title.clone())
            .link(film.url.clone())
            .description(description)
            .guid(guid);
        if let Some(date) = pub_date {
            item_builder.pub_date(date);
        }
        items.push(item_builder.build());
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

/// Generate a single RSS feed from multiple cinemas. Each item has a category set to the cinema name.
pub fn generate_rss_merged(
    channel_title: &str,
    channel_link: &str,
    channel_description: &str,
    sources: &[(&str, &[Film])],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for (cinema_name, films) in sources {
        let category = Category {
            name: (*cinema_name).to_string(),
            domain: None,
        };
        for film in *films {
            let (description, pub_date) = film_description_and_pub_date(film);
            let guid = rss::Guid {
                value: film.url.clone(),
                permalink: true,
            };
            let mut item_builder = ItemBuilder::default();
            item_builder
                .title(film.title.clone())
                .link(film.url.clone())
                .description(description)
                .guid(guid)
                .categories(vec![category.clone()]);
            if let Some(date) = pub_date {
                item_builder.pub_date(date);
            }
            items.push(item_builder.build());
        }
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
