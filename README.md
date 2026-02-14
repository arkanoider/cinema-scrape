# cinema-scrape

A Rust-based scraper that collects film schedules from independent and art-house cinemas and turns them into clean, subscribable RSS feeds -- updated daily via GitHub Actions.

---

## Why this exists

I love cinema. Real cinema.

After COVID, going back to the movies felt like walking into a theme park. Every screen was taken over by Marvel, DC, and the endless parade of franchise sequels. Finding a screening of something with actual storytelling, a human director's vision, or just *something different* became an exhausting treasure hunt across dozens of cinema websites.

So I built this. A simple scraper that monitors local cinemas -- the ones that still believe film is an art form, not just a product -- and packages their schedules into clean RSS feeds I can subscribe to from my phone. No more clicking through ten websites every week. Just open the feed reader, see what's playing, and go.

## What it does

- **Scrapes** film schedules from 13 cinemas and festivals (mostly in northeast Italy, plus a couple of international gems)
- **Generates RSS feeds** with full film details: title, synopsis, cast, poster, showtimes
- **Auto-updates daily** at 06:00 UTC via GitHub Actions
- **Serves feeds** through GitHub Pages -- subscribe once, stay updated forever

## Supported cinemas

| Feed | Cinemas |
|------|---------|
| **multisala.xml** | The Space Cinema (Silea), Cinema Multisala Edera, Cinema Multisala Manzoni, Cinergia Conegliano, Cinemazero Pordenone |
| **padova.xml** | Cinema Rex Padova, Cinema Porto Astra |
| **trieste.xml** | Cinema Ariston Trieste (La Cappella Underground) |
| **rassegne.xml** | Rassegne from Cinema Cristallo Oderzo, Cinema Edera, Circolo Enrico Pizzuti |
| **berlinale.xml** | Berlinale (Berlin International Film Festival) |
| **tarantino.xml** | The New Beverly Cinema (Quentin Tarantino's revival theater, Los Angeles) |

See [FEEDS.md](FEEDS.md) for full feed URLs and setup instructions.

## Tech stack

- **Rust** (edition 2024) -- fast, safe, and reliable for long-running scrapers
- **reqwest** -- async HTTP client with cookie support
- **scraper** -- HTML parsing with CSS selectors
- **rss** -- RSS 2.0 feed generation
- **chrono** -- date/time handling
- **tokio** -- async runtime
- **GitHub Actions** -- daily automated feed updates
- **GitHub Pages** -- zero-cost feed hosting

## Build & run

```bash
# Build
cargo build --release

# Run (generates all feeds in docs/feeds/)
cargo run --release
```

Feeds are written to `docs/feeds/` and served via GitHub Pages.

## Get involved

Do you know an independent cinema, a film festival, or an interesting project about movies that deserves more visibility? I'd love to hear about it.

Whether it's a small art-house theater in your city, a local film rassegna, or a niche festival that celebrates cinema as an art form -- **open an issue** or reach out. If the cinema has a website with a schedule, chances are we can scrape it and add it to the feeds.

The world needs fewer algorithms and more curated film experiences.

## License

This project is open source. Feel free to fork, adapt, and build your own cinema feeds.
