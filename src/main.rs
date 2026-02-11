mod cinema_edera;
mod cinemazero;
mod enrico_pizzuti;
mod rassegne;
mod space_cinema;

use cinema_edera::CinemaEderaScraper;
use cinema_scrape::{CinemaScraper, Film, generate_rss, generate_rss_merged};
use cinemazero::CinemazeroScraper;
use enrico_pizzuti::EnricoPizzutiScraper;
use rassegne::RassegneScraper;
use space_cinema::SpaceCinemaScraper;
use std::fs;

fn print_films(films: &[Film]) {
    for film in films {
        println!("TITLE       : {}", film.title);
        println!("URL         : {}", film.url);
        if let Some(ref poster) = film.poster_url {
            println!("POSTER      : {}", poster);
        }
        if let Some(ref cast) = film.cast {
            println!("CAST        : {}", cast);
        }
        if let Some(ref date) = film.release_date {
            println!("RELEASE DATE: {}", date);
        }
        if let Some(time) = film.running_time {
            println!("RUNTIME     : {} min", time);
        }
        if let Some(ref synopsis) = film.synopsis {
            println!("SYNOPSIS    : {}", synopsis);
        }
        if let Some(ref showtimes) = film.showtimes {
            for s in showtimes {
                println!("ORARIO      : {}", s);
            }
        }
        println!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a client with cookie store
    let client = reqwest::Client::builder().cookie_store(true).build()?;

    // SHOWING_DATE env (e.g. 2026-02-09T00:00:00) or today for scheduled runs (Space Cinema)
    let showing_date = std::env::var("SHOWING_DATE")
        .unwrap_or_else(|_| chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string());

    let space_scraper = SpaceCinemaScraper::new(1009, showing_date);
    let edera_scraper = CinemaEderaScraper::new(
        "https://www.cinemaedera.it/i-film-della-settimana.html".to_string(),
    );
    let pizzuti_scraper = EnricoPizzutiScraper::new("https://www.enricopizzuti.it/".to_string());
    let cinemazero_scraper = CinemazeroScraper::new("https://cinemazero.it/".to_string());
    let rassegne_scraper =
        RassegneScraper::new("https://www.cinemacristallo.com/rassegna-film-dautore/".to_string());

    // Fetch all cinemas (names used as categories in the merged feed)
    const SPACE_NAME: &str = "The Space Cinema - Silea";
    const EDERA_NAME: &str = "Cinema Multisala Edera";
    const PIZZUTI_NAME: &str = "Circolo Enrico Pizzuti";
    const CINEMAZERO_NAME: &str = "Cinemazero Pordenone";

    println!("=== Fetching from The Space Cinema ===\n");
    space_scraper.warm_up(&client).await?;
    let space_films = space_scraper.fetch_films(&client).await.unwrap_or_default();
    print_films(&space_films);

    println!("\n=== Fetching from Cinema Edera ===\n");
    let edera_films = edera_scraper.fetch_films(&client).await.unwrap_or_default();
    print_films(&edera_films);

    println!("\n=== Fetching from Circolo Enrico Pizzuti ===\n");
    let pizzuti_films = pizzuti_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&pizzuti_films);

    println!("\n=== Fetching from Cinemazero Pordenone ===\n");
    let cinemazero_films = cinemazero_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&cinemazero_films);

    println!("\n=== Fetching from Cinema Cristallo Oderzo - Rassegna Film d’Autore ===\n");
    let rassegne_films = rassegne_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&rassegne_films);

    // Separate RSS feed just for the Rassegna Film d’Autore.
    let rassegne_rss_xml = generate_rss(
        &rassegne_films,
        "Rassegna Film d’Autore",
        "https://www.cinemacristallo.com/rassegna-film-dautore/",
        "Rassegna Film d’Autore",
    )?;
    let rassegne_feed_path = rassegne_scraper.rss_filename();
    fs::write(&rassegne_feed_path, rassegne_rss_xml)?;
    println!("✓ Rassegne RSS feed saved to: {}", rassegne_feed_path);

    // Single merged RSS with cinema names as categories (Rassegne excluded)
    let rss_xml = generate_rss_merged(
        "Film in programmazione",
        "https://github.com/", // optional: set to your repo or a landing page
        "RSS unificato: The Space Cinema (Silea), Cinema Multisala Edera, Circolo Enrico Pizzuti, Cinemazero Pordenone.",
        &[
            (SPACE_NAME, space_films.as_slice()),
            (EDERA_NAME, edera_films.as_slice()),
            (PIZZUTI_NAME, pizzuti_films.as_slice()),
            (CINEMAZERO_NAME, cinemazero_films.as_slice()),
        ],
    )?;
    let feed_path = "feeds.xml";
    fs::write(feed_path, rss_xml)?;
    println!("✓ Merged RSS feed saved to: {}", feed_path);
    Ok(())
}
