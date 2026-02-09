mod cinema_edera;
mod enrico_pizzuti;
mod space_cinema;

use cinema_scrape::{generate_rss, CinemaScraper, Film};
use cinema_edera::CinemaEderaScraper;
use enrico_pizzuti::EnricoPizzutiScraper;
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
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()?;

    // Example 1: Space Cinema (JSON API)
    println!("=== Fetching from The Space Cinema ===\n");
    let space_scraper = SpaceCinemaScraper::new(1009, "2026-02-09T00:00:00".to_string());
    space_scraper.warm_up(&client).await?;
    match space_scraper.fetch_films(&client).await {
        Ok(films) => {
            print_films(&films);
            // Generate RSS feed
            let rss_xml = generate_rss(
                &films,
                "The Space Cinema - Silea",
                "https://www.thespacecinema.it/cinema/silea/al-cinema",
                "Film in programmazione al The Space Cinema di Silea",
            )?;
            let filename = space_scraper.rss_filename();
            fs::write(&filename, rss_xml)?;
            println!("✓ RSS feed saved to: {}\n", filename);
        }
        Err(e) => eprintln!("Error fetching Space Cinema: {}", e),
    }

    // Example 2: Cinema Edera (HTML scraping)
    println!("\n=== Fetching from Cinema Edera ===\n");
    let edera_scraper = CinemaEderaScraper::new(
        "https://www.cinemaedera.it/i-film-della-settimana.html".to_string(),
    );
    match edera_scraper.fetch_films(&client).await {
        Ok(films) => {
            print_films(&films);
            // Generate RSS feed
            let rss_xml = generate_rss(
                &films,
                "Cinema Multisala Edera",
                "https://www.cinemaedera.it/i-film-della-settimana.html",
                "Film in programmazione al Cinema Multisala Edera",
            )?;
            let filename = edera_scraper.rss_filename();
            fs::write(&filename, rss_xml)?;
            println!("✓ RSS feed saved to: {}\n", filename);
        }
        Err(e) => eprintln!("Error fetching Cinema Edera: {}", e),
    }

    // Example 3: Circolo Enrico Pizzuti (HTML scraping)
    println!("\n=== Fetching from Circolo Enrico Pizzuti ===\n");
    let pizzuti_scraper =
        EnricoPizzutiScraper::new("https://www.enricopizzuti.it/".to_string());
    match pizzuti_scraper.fetch_films(&client).await {
        Ok(films) => {
            print_films(&films);
            // Generate RSS feed
            let rss_xml = generate_rss(
                &films,
                "Circolo Cinematografico Enrico Pizzuti",
                "https://www.enricopizzuti.it/",
                "Film in programmazione al Circolo Cinematografico Enrico Pizzuti - Cinema Turroni Oderzo",
            )?;
            let filename = pizzuti_scraper.rss_filename();
            fs::write(&filename, rss_xml)?;
            println!("✓ RSS feed saved to: {}\n", filename);
        }
        Err(e) => eprintln!("Error fetching Enrico Pizzuti: {}", e),
    }

    println!("=== All RSS feeds generated successfully! ===");
    Ok(())
}
