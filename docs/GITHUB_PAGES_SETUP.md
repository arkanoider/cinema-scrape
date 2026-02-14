# GitHub Pages Setup Guide

This guide explains how to enable GitHub Pages for your RSS feeds.

## Quick Setup

1. **Push the changes** - Make sure all code changes are committed and pushed to your repository.

2. **Enable GitHub Pages**:
   - Go to your repository on GitHub
   - Click **Settings** → **Pages** (in the left sidebar)
   - Under **Source**, select **Deploy from a branch**
   - Choose **main** (or your default branch) and **/docs** folder
   - Click **Save**

3. **Wait for deployment** - GitHub Pages typically takes 1-2 minutes to deploy. You'll see a green checkmark when it's ready.

4. **Access your feeds** - Your feeds will be available at:
   - `https://arkanoider.github.io/cinema-scrape/feeds/multisala.xml`
   - `https://arkanoider.github.io/cinema-scrape/feeds/padova.xml`
   - `https://arkanoider.github.io/cinema-scrape/feeds/trieste.xml`
   - `https://arkanoider.github.io/cinema-scrape/feeds/rassegne.xml`
   - `https://arkanoider.github.io/cinema-scrape/feeds/berlinale.xml`
   - `https://arkanoider.github.io/cinema-scrape/feeds/tarantino.xml`

## How It Works

- GitHub Pages serves files from the `docs/` folder in your repository
- The RSS feeds are generated in `docs/feeds/` by the GitHub Actions workflow
- Each time the workflow runs, it updates the feed files and commits them
- GitHub Pages automatically rebuilds and serves the updated content

## Troubleshooting

**Feeds not accessible?**
- Check that GitHub Pages is enabled in Settings → Pages
- Verify the source is set to `/docs` folder
- Wait a few minutes after enabling (first deployment can take time)
- Check the Actions tab to ensure the workflow completed successfully

**404 errors?**
- Make sure the feed files exist in `docs/feeds/`
- Verify the file names match exactly (case-sensitive)
- Check that the workflow successfully committed the files

**Custom domain?**
- You can add a custom domain in Settings → Pages → Custom domain
- This requires DNS configuration on your domain provider

## Benefits of GitHub Pages

- **Cleaner URLs**: `username.github.io/repo/feeds/multisala.xml` vs `raw.githubusercontent.com/...`
- **Better caching**: GitHub Pages CDN provides better caching than raw files
- **HTTPS by default**: All GitHub Pages sites use HTTPS
- **No rate limits**: Unlike raw.githubusercontent.com API, Pages has no rate limits
- **Can add HTML**: You can add an `index.html` in `docs/` to create a landing page
