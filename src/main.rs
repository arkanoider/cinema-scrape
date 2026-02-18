mod berlinale;
mod cinema_edera;
mod cinema_padova;
mod cinema_trieste_scraper;
mod cinemazero;
mod cinergia_conegliano;
mod enrico_pizzuti;
mod new_bev;
mod porto_astra;
mod rassegne_cristallo;
mod rassegne_edera;
mod space_cinema;

use berlinale::BerlinaleScraper;
use cinema_edera::CinemaEderaScraper;
use cinema_padova::FeedPadovaScraper;
use cinema_scrape::{CinemaScraper, Film, generate_rss, generate_rss_merged};
use cinema_trieste_scraper::CinemaTriesteScraper;
use cinemazero::CinemazeroScraper;
use cinergia_conegliano::CinergiaConeglianoScraper;
use clap::{Parser, ValueEnum};
use enrico_pizzuti::EnricoPizzutiScraper;
use new_bev::NewBevScraper;
use porto_astra::PortoAstraScraper;
use rassegne_cristallo::RassegneScraperCristallo;
use rassegne_edera::RassegneScraperEdera;
use space_cinema::SpaceCinemaScraper;
use std::fs;

/// Which single feed to generate. If omitted, all feeds are generated.
#[derive(Clone, PartialEq, ValueEnum)]
#[value(rename_all = "lowercase")]
enum Feed {
    Multisala,
    Padova,
    Trieste,
    Rassegne,
    Berlinale,
    Tarantino,
}

#[derive(Parser)]
struct Args {
    /// Generate only this feed (default: all feeds)
    #[arg(long)]
    feed: Option<Feed>,
}

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
    fs::create_dir_all("docs/feeds")?;
    let client = reqwest::Client::builder().cookie_store(true).build()?;
    let feed_filter = Args::parse().feed;

    const SPACE_NAME: &str = "The Space Cinema - Silea";
    const EDERA_NAME: &str = "Cinema Multisala Edera";
    const MANZONI_NAME: &str = "Cinema Multisala Manzoni";
    const CINERGIA_NAME: &str = "Cinergia Conegliano";
    const PIZZUTI_NAME: &str = "Circolo Enrico Pizzuti";
    const CINEMAZERO_NAME: &str = "Cinemazero Pordenone";

    // --- multisala ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Multisala) {
        let showing_date = std::env::var("SHOWING_DATE")
            .unwrap_or_else(|_| chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string());
        let space_scraper = SpaceCinemaScraper::new(1009, showing_date);
        let edera_scraper = CinemaEderaScraper::new(
            "https://www.cinemaedera.it/i-film-della-settimana.html".to_string(),
        );
        let manzoni_scraper = CinemaEderaScraper::new(
            "https://www.cinemamanzoni.it/i-film-della-settimana.html".to_string(),
        );
        let cinergia_scraper =
            CinergiaConeglianoScraper::new("https://coneglianocinergia.18tickets.it/".to_string());
        let cinemazero_scraper =
            CinemazeroScraper::new("https://cinemazero.it/programmazione/".to_string());

        println!("=== Fetching from The Space Cinema ===\n");
        space_scraper.warm_up(&client).await?;
        let space_films = space_scraper.fetch_films(&client).await.unwrap_or_default();
        print_films(&space_films);

        println!("\n=== Fetching from Cinema Edera ===\n");
        let edera_films = edera_scraper.fetch_films(&client).await.unwrap_or_default();
        print_films(&edera_films);

        println!("\n=== Fetching from Cinema Manzoni ===\n");
        let manzoni_films = manzoni_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&manzoni_films);

        println!("\n=== Fetching from Cinergia Conegliano ===\n");
        let cinergia_films = cinergia_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&cinergia_films);

        println!("\n=== Fetching from Cinemazero Pordenone ===\n");
        let cinemazero_films = cinemazero_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&cinemazero_films);

        let rss_xml = generate_rss_merged(
            "Film in programmazione",
            "https://github.com/",
            "RSS unificato: The Space Cinema (Silea), Cinema Multisala Edera, Cinema Manzoni, Cinergia Conegliano, Cinemazero Pordenone.",
            &[
                (SPACE_NAME, space_films.as_slice()),
                (EDERA_NAME, edera_films.as_slice()),
                (MANZONI_NAME, manzoni_films.as_slice()),
                (CINERGIA_NAME, cinergia_films.as_slice()),
                (CINEMAZERO_NAME, cinemazero_films.as_slice()),
            ],
        )?;
        let feed_path = "docs/feeds/multisala.xml";
        fs::write(feed_path, rss_xml)?;
        println!("✓ Merged RSS feed saved to: {}", feed_path);
    }

    // --- padova ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Padova) {
        let padova_scraper =
            FeedPadovaScraper::new("https://www.cinemarex.it/programmazione".to_string());
        let porto_astra_scraper =
            PortoAstraScraper::new("https://portoastra.it/questa-settimana/".to_string());

        println!("\n=== Fetching from Cinema Rex Padova ===\n");
        let padova_films = match padova_scraper.fetch_films(&client).await {
            Ok(films) => films,
            Err(e) => {
                eprintln!("Error fetching Cinema Rex Padova films: {e}");
                Vec::new()
            }
        };
        print_films(&padova_films);

        println!("\n=== Fetching from Cinema Porto Astra Padova ===\n");
        let porto_astra_films = porto_astra_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&porto_astra_films);

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
    }

    // --- trieste ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Trieste) {
        let trieste_scraper = CinemaTriesteScraper::new();

        println!("\n=== Fetching from Cinema Ariston Trieste (La Cappella Underground) ===\n");
        let trieste_films = trieste_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&trieste_films);

        let trieste_rss_xml = generate_rss(
            &trieste_films,
            "Cinema Ariston Trieste - La Cappella Underground",
            "https://www.lacappellaunderground.org/ariston/programma/",
            "Programmazione Cinema Ariston - La Cappella Underground",
        )?;
        let trieste_feed_path = trieste_scraper.rss_filename();
        fs::write(&trieste_feed_path, trieste_rss_xml)?;
        println!("✓ Trieste RSS feed saved to: {}", trieste_feed_path);
    }

    // --- rassegne ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Rassegne) {
        let rassegne_scraper = RassegneScraperCristallo::new(
            "https://www.cinemacristallo.com/rassegna-film-dautore/".to_string(),
        );
        let edera_rassegne_scraper =
            RassegneScraperEdera::new("https://www.cinemaedera.it/rassegne.html".to_string());
        let pizzuti_scraper =
            EnricoPizzutiScraper::new("https://www.enricopizzuti.it/".to_string());

        println!("\n=== Fetching from Cinema Cristallo Oderzo - Rassegna Film d'Autore ===\n");
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

        println!("\n=== Fetching from Circolo Enrico Pizzuti ===\n");
        let pizzuti_films = pizzuti_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&pizzuti_films);

        let rassegne_rss_xml = generate_rss_merged(
            "Rassegne",
            "https://github.com/",
            "Rassegne di Cinema Cristallo Oderzo, Cinema Edera e Circolo Enrico Pizzuti.",
            &[
                ("Cinema Cristallo Oderzo", rassegne_films.as_slice()),
                ("Cinema Edera", edera_rassegne_films.as_slice()),
                (PIZZUTI_NAME, pizzuti_films.as_slice()),
            ],
        )?;
        let rassegne_feed_path = rassegne_scraper.rss_filename();
        fs::write(&rassegne_feed_path, rassegne_rss_xml)?;
        println!("✓ Rassegne RSS feed saved to: {}", rassegne_feed_path);
    }

    // --- berlinale ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Berlinale) {
        let berlinale_scraper = BerlinaleScraper::new(
            "https://www.berlinale.de/en/programme/on-sale-from-today.html".to_string(),
        );

        println!("\n=== Fetching from Berlinale ===\n");
        let berlinale_films = berlinale_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&berlinale_films);

        let berlinale_rss_xml = generate_rss(
            &berlinale_films,
            "Berlinale - Berlin International Film Festival",
            "https://www.berlinale.de/en/programme/on-sale-from-today.html",
            "Films in the Berlinale programme (on sale / in programme).",
        )?;
        let berlinale_feed_path = berlinale_scraper.rss_filename();
        fs::write(&berlinale_feed_path, berlinale_rss_xml)?;
        println!("✓ Berlinale RSS feed saved to: {}", berlinale_feed_path);
    }

    // --- tarantino ---
    if feed_filter.is_none() || feed_filter.as_ref() == Some(&Feed::Tarantino) {
        let new_bev_scraper = NewBevScraper::new();

        println!("\n=== Fetching from The New Beverly Cinema ===\n");
        let new_bev_films = new_bev_scraper
            .fetch_films(&client)
            .await
            .unwrap_or_default();
        print_films(&new_bev_films);

        let new_bev_rss_xml = generate_rss(
            &new_bev_films,
            "The New Beverly Cinema",
            "https://thenewbev.com/schedule/",
            "Schedule and program for The New Beverly Cinema (Quentin Tarantino's revival theater in Los Angeles).",
        )?;
        let new_bev_feed_path = new_bev_scraper.rss_filename();
        fs::write(&new_bev_feed_path, new_bev_rss_xml)?;
        println!("✓ New Beverly RSS feed saved to: {}", new_bev_feed_path);
    }

    Ok(())
}
