# Hosted RSS feeds

All RSS feeds are located in the `docs/feeds/` directory and are served via **GitHub Pages**. Replace `YOUR_USERNAME` and `YOUR_REPO` with your GitHub username and repo name:

## Available feeds (GitHub Pages)

**Main feed (all cinemas merged):**
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/multisala.xml`
  - Includes: The Space Cinema (Silea), Cinema Multisala Edera, Cinema Multisala Manzoni, Cinergia Conegliano, Cinemazero Pordenone
  - Each item has a **category** with the cinema name so you can filter in your reader.

**Regional feeds:**
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/padova.xml` - Cinema Rex Padova + Cinema Porto Astra
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/trieste.xml` - Cinema Ariston Trieste (La Cappella Underground)

**Special feeds:**
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/rassegne.xml` - Rassegne from Cinema Cristallo Oderzo, Cinema Edera e Circolo Enrico Pizzuti

**Festival:**
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/berlinale.xml` - Berlinale (Berlin International Film Festival) programme

**Cinema:**
- `https://YOUR_USERNAME.github.io/YOUR_REPO/feeds/tarantino.xml` - The New Beverly Cinema (Quentin Tarantino's revival theater, Los Angeles)

### Alternative: Raw GitHub URLs

Feeds are also available via raw GitHub URLs (works without GitHub Pages):
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/multisala.xml`
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/padova.xml`
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/trieste.xml`
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/rassegne.xml`
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/berlinale.xml`
- `https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/docs/feeds/tarantino.xml`

| Category in main feed | Cinema |
|----------------------|--------|
| The Space Cinema - Silea | Space Cinema (Silea) |
| Cinema Multisala Edera | Cinema Edera |
| Cinema Multisala Manzoni | Cinema Manzoni (Paese, TV) |
| Cinergia Conegliano | Cinergia Conegliano |
| Cinemazero Pordenone | Cinemazero Pordenone |

## Auto-update

The **Update RSS feeds** GitHub Action runs daily (06:00 UTC) and on manual trigger. It builds the scraper, generates all feeds in the `docs/feeds/` directory, and commits them so the URLs above stay up to date.

- Commit and push the feed files once so they exist in the repo.
- After that, the workflow will refresh them automatically (or run it manually from the **Actions** tab).

## Preview the feeds

**Local (before or without hosting):**
- **Firefox**: Open any feed file directly (e.g. `file:///path/to/cinema-scrape/docs/feeds/multisala.xml`). Firefox shows a readable RSS-style preview.
- **RSS readers**: Many desktop apps (e.g. Thunderbird, NetNewsWire) let you add a feed from a local file path.
- **Serve locally**: From the project folder run `python3 -m http.server 8080`, then open `http://localhost:8080/docs/feeds/multisala.xml` in Firefox for a preview.

**Online (once the feed is on GitHub):**
- **W3C Feed Validator**: [https://validator.w3.org/feed/](https://validator.w3.org/feed/) — paste your raw feed URL (or paste the XML). Validates and shows a readable preview.
- **RSS readers**: Subscribe with the raw URL in [Feedly](https://feedly.com), [Inoreader](https://www.inoreader.com), or any app that accepts a feed URL.
- **rss.app**: [https://rss.app](https://rss.app) — paste the feed URL to preview and share.
