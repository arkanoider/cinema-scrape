use reqwest::Client;

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
}
