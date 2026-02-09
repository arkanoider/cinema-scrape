use reqwest::header;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ApiResponse {
    result: Vec<Film>,
}

#[derive(Debug, Deserialize)]
struct Film {
    filmUrl: String,
    filmAttributes: Vec<serde_json::Value>,
    posterImageSrc: String,
    cast: String,
    releaseDate: String,
    runningTime: i32,
    synopsisShort: String,
    filmTitle: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_url = "https://www.thespacecinema.it/api/microservice/showings/cinemas/1009/films";

    // Build a client with cookie store so it remembers Set-Cookie headers.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()?;

    // 1) Warm-up request: hit a normal page to obtain fresh cookies/tokens.
    client
        .get("https://www.thespacecinema.it/")
        .header(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/143.0.0.0 Safari/537.36",
        )
        .send()
        .await?
        .error_for_status()?;

    // 2) Now call the JSON API; cookies are attached automatically.
    let resp = client
        .get(api_url)
        .header(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/143.0.0.0 Safari/537.36",
        )
        .header(
            header::ACCEPT,
            "application/json,text/javascript,*/*;q=0.1",
        )
        .query(&[
            ("showingDate", "2026-02-09T00:00:00"),
            ("minEmbargoLevel", "3"),
            ("includesSession", "true"),
            ("includeSessionAttributes", "true"),
        ])
        .send()
        .await?
        .error_for_status()?;

    let body = resp.text().await?;

    let parsed: ApiResponse = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse JSON: {e}");
            eprintln!("Raw response body (first 500 chars):");
            let preview: String = body.chars().take(500).collect();
            eprintln!("{}", preview);
            return Ok(());
        }
    };

    for film in parsed.result {
        println!("TITLE       : {}", film.filmTitle);
        println!("URL         : {}", film.filmUrl);
        println!("POSTER      : {}", film.posterImageSrc);
        println!("CAST        : {}", film.cast);
        println!("RELEASE DATE: {}", film.releaseDate);
        println!("RUNTIME     : {} min", film.runningTime);
        println!("SYNOPSIS    : {}", film.synopsisShort);
        println!();
    }

    Ok(())
}
