mod cinema_edera;
mod cinema_padova;
mod cinema_trieste_scraper;
mod cinemazero;
mod enrico_pizzuti;
mod porto_astra;
mod rassegne_cristallo;
mod rassegne_edera;
mod space_cinema;

use cinema_edera::CinemaEderaScraper;
use cinema_padova::FeedPadovaScraper;
use cinema_scrape::{CinemaScraper, Film, generate_rss, generate_rss_merged};
use cinema_trieste_scraper::CinemaTriesteScraper;
use cinemazero::CinemazeroScraper;
use enrico_pizzuti::EnricoPizzutiScraper;
use porto_astra::PortoAstraScraper;
use rassegne_cristallo::RassegneScraperCristallo;
use rassegne_edera::RassegneScraperEdera;
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
    let rassegne_scraper = RassegneScraperCristallo::new(
        "https://www.cinemacristallo.com/rassegna-film-dautore/".to_string(),
    );
    let edera_rassegne_scraper =
        RassegneScraperEdera::new("https://www.cinemaedera.it/rassegne.html".to_string());
    let padova_scraper =
        FeedPadovaScraper::new("https://www.cinemarex.it/programmazione".to_string());

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

    println!("\n=== Fetching from Cinema Edera - Rassegne ===\n");
    let edera_rassegne_films = edera_rassegne_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&edera_rassegne_films);

    println!("\n=== Fetching from Cinema Rex Padova ===\n");
    let padova_films = padova_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&padova_films);

    println!("\n=== Fetching from Cinema Porto Astra Padova ===\n");
    let porto_astra_scraper =
        PortoAstraScraper::new("https://portoastra.it/questa-settimana/".to_string());
    let porto_astra_films = porto_astra_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&porto_astra_films);

    // Merged Padova RSS feed (Cinema Rex + Porto Astra),
    // with per-item cinema names in title and category.
    let padova_rss_xml = generate_rss_merged(
        "Film in programmazione a Padova",
        "https://portoastra.it/questa-settimana/",
        "Programmazione Cinema Rex Padova e Cinema Porto Astra.",
        &[
            ("Cinema Rex Padova", padova_films.as_slice()),
            ("Cinema Porto Astra", porto_astra_films.as_slice()),
        ],
    )?;
    let padova_feed_path = padova_scraper.rss_filename();
    fs::write(&padova_feed_path, padova_rss_xml)?;
    println!("✓ Padova RSS feed saved to: {}", padova_feed_path);

    println!("\n=== Fetching from Cinema Ariston Trieste (La Cappella Underground) ===\n");
    let trieste_scraper = CinemaTriesteScraper::new();
    let trieste_films = trieste_scraper
        .fetch_films(&client)
        .await
        .unwrap_or_default();
    print_films(&trieste_films);

    // Trieste RSS feed (single cinema)
    let trieste_rss_xml = generate_rss(
        &trieste_films,
        "Cinema Ariston Trieste - La Cappella Underground",
        "https://www.lacappellaunderground.org/ariston/programma/",
        "Programmazione Cinema Ariston - La Cappella Underground",
    )?;
    let trieste_feed_path = trieste_scraper.rss_filename();
    fs::write(&trieste_feed_path, trieste_rss_xml)?;
    println!("✓ Trieste RSS feed saved to: {}", trieste_feed_path);

    // Single RSS feed for all rassegne (Cristallo + Edera),
    // with cinema names included per item via categories and titles.
    let rassegne_rss_xml = generate_rss_merged(
        "Rassegne",
        "https://github.com/", // optional: landing page
        "Rassegne di Cinema Cristallo Oderzo e Cinema Edera.",
        &[
            ("Cinema Cristallo Oderzo", rassegne_films.as_slice()),
            ("Cinema Edera", edera_rassegne_films.as_slice()),
        ],
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
