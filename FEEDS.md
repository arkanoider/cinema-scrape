# Hosted RSS feed

A **single RSS file** includes all three cinemas. Each item has a **category** with the cinema name so you can filter in your reader.

Replace `YOUR_USERNAME` and `YOUR_REPO` with your GitHub username and repo name, and `main` with your default branch if different:

**Feed URL (all cinemas):**

`https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_REPO/main/feeds.xml`

| Category in feed | Cinema |
|------------------|--------|
| The Space Cinema - Silea | Space Cinema (Silea) |
| Cinema Multisala Edera | Cinema Edera |
| Circolo Enrico Pizzuti | Enrico Pizzuti |

## Auto-update

The **Update RSS feeds** GitHub Action runs daily (06:00 UTC) and on manual trigger. It builds the scraper, generates `feeds.xml`, and commits it so the URL above stays up to date.

- Commit and push `feeds.xml` once so it exists in the repo.
- After that, the workflow will refresh it automatically (or run it manually from the **Actions** tab).

## Preview the feed

**Local (before or without hosting):**
- **Firefox**: Open `feeds.xml` directly (e.g. `file:///path/to/cinema-scrape/feeds.xml`). Firefox shows a readable RSS-style preview.
- **RSS readers**: Many desktop apps (e.g. Thunderbird, NetNewsWire) let you add a feed from a local file path.
- **Serve locally**: From the project folder run `python3 -m http.server 8080`, then open `http://localhost:8080/feeds.xml` in Firefox for a preview.

**Online (once the feed is on GitHub):**
- **W3C Feed Validator**: [https://validator.w3.org/feed/](https://validator.w3.org/feed/) — paste your raw feed URL (or paste the XML). Validates and shows a readable preview.
- **RSS readers**: Subscribe with the raw URL in [Feedly](https://feedly.com), [Inoreader](https://www.inoreader.com), or any app that accepts a feed URL.
- **rss.app**: [https://rss.app](https://rss.app) — paste the feed URL to preview and share.
