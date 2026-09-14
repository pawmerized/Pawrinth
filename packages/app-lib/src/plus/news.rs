//! Pawrinth's own announcements, read from a plain JSON file the project publishes.
//! There's no news service. A build with no feed configured simply has no news, and
//! nothing here ever makes up an article.

use crate::ErrorKind;
use anyhow::bail;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

#[cfg(debug_assertions)]
const FEED_URL_ENV: &str = "PAWRINTH_NEWS_FEED_URL";
/// Pawrinth's own feed, a file in its GitHub repository. A build can name another
/// with `PAWRINTH_NEWS_FEED_URL`.
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/pawmerized/Pawrinth/main/news/news.json";
const MAX_FEED_BYTES: usize = 512 * 1024;
const MAX_ARTICLES: usize = 20;
const MAX_TITLE_CHARS: usize = 160;
const MAX_SUMMARY_CHARS: usize = 600;
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum NewsCategory {
    Release,
    Update,
    Feature,
    Maintenance,
    Compatibility,
    Community,
    #[default]
    #[serde(other)]
    Announcement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NewsArticle {
    pub title: String,
    pub summary: String,
    pub date: String,
    pub link: String,
    pub thumbnail: Option<String>,
    pub category: NewsCategory,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NewsFeed {
    /// `false` when this build has no feed, which is different from a feed with
    /// nothing in it yet.
    pub configured: bool,
    pub articles: Vec<NewsArticle>,
    pub all_news_url: Option<String>,
}

#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    articles: Vec<serde_json::Value>,
    all_news_url: Option<String>,
}

#[derive(Deserialize)]
struct RawArticle {
    title: String,
    #[serde(default)]
    summary: String,
    date: String,
    link: String,
    thumbnail: Option<String>,
    #[serde(default)]
    category: NewsCategory,
}

/// Development builds can point at a local file server to test a populated feed.
#[cfg(debug_assertions)]
fn local_feed_url() -> Option<String> {
    std::env::var(FEED_URL_ENV)
        .ok()
        .filter(|url| url.starts_with("http://127.0.0.1:"))
}

fn feed_url() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Some(url) = local_feed_url() {
        return Some(url);
    }
    Some(
        option_env!("PAWRINTH_NEWS_FEED_URL")
            .filter(|url| is_https(url))
            .unwrap_or(DEFAULT_FEED_URL)
            .to_string(),
    )
}

fn http_client() -> &'static reqwest::Client {
    #[cfg(debug_assertions)]
    if local_feed_url().is_some() {
        return &crate::util::fetch::INSECURE_REQWEST_CLIENT;
    }
    &crate::util::fetch::REQWEST_CLIENT
}

fn is_https(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| parsed.scheme() == "https")
}

fn normalized_date(date: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(date)
        .map(|parsed| parsed.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)
                .map(|parsed| parsed.and_utc())
        })
}

/// One bad entry is dropped rather than losing the whole feed.
fn parse_article(
    value: serde_json::Value,
) -> Option<(DateTime<Utc>, NewsArticle)> {
    let raw: RawArticle = serde_json::from_value(value).ok()?;
    let title = raw.title.trim().to_string();
    let summary = raw.summary.trim().to_string();
    let date = normalized_date(raw.date.trim())?;
    let valid = !title.is_empty()
        && title.chars().count() <= MAX_TITLE_CHARS
        && summary.chars().count() <= MAX_SUMMARY_CHARS
        && is_https(&raw.link);
    valid.then(|| {
        (
            date,
            NewsArticle {
                title,
                summary,
                date: date.to_rfc3339(),
                link: raw.link,
                thumbnail: raw.thumbnail.filter(|url| is_https(url)),
                category: raw.category,
            },
        )
    })
}

fn parse_feed(bytes: &[u8]) -> Result<NewsFeed, serde_json::Error> {
    let raw: RawFeed = serde_json::from_slice(bytes)?;
    let mut dated: Vec<_> =
        raw.articles.into_iter().filter_map(parse_article).collect();
    dated.sort_by_key(|(date, _)| std::cmp::Reverse(*date));
    let mut seen = HashSet::new();
    let articles = dated
        .into_iter()
        .map(|(_, article)| article)
        .filter(|article| seen.insert(article.link.clone()))
        .take(MAX_ARTICLES)
        .collect();
    Ok(NewsFeed {
        configured: true,
        articles,
        all_news_url: raw.all_news_url.filter(|url| is_https(url)),
    })
}

/// A feed file that hasn't been published yet means no news so far, not a failure.
fn unpublished_feed() -> NewsFeed {
    NewsFeed {
        configured: true,
        ..NewsFeed::default()
    }
}

pub async fn feed() -> crate::Result<NewsFeed> {
    let Some(url) = feed_url() else {
        return Ok(NewsFeed::default());
    };
    let failed = || {
        ErrorKind::InputError("Pawrinth News couldn't be loaded".to_string())
    };
    let response = http_client()
        .get(&url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|_| failed())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(unpublished_feed());
    }
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_FEED_BYTES as u64)
    {
        bail!(failed());
    }
    let bytes = response.bytes().await.map_err(|_| failed())?;
    if bytes.len() > MAX_FEED_BYTES {
        bail!(failed());
    }
    Ok(parse_feed(&bytes).map_err(|_| failed())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(title: &str, date: &str, link: &str) -> serde_json::Value {
        serde_json::json!({ "title": title, "summary": "Text", "date": date, "link": link })
    }

    fn parse(articles: Vec<serde_json::Value>) -> NewsFeed {
        let raw = serde_json::json!({ "articles": articles });
        parse_feed(raw.to_string().as_bytes()).unwrap()
    }

    #[test]
    fn newest_comes_first_and_both_date_forms_work() {
        let feed = parse(vec![
            article("Old", "2026-01-01", "https://example.invalid/a"),
            article("New", "2026-06-01T10:00:00Z", "https://example.invalid/b"),
        ]);
        assert_eq!(feed.articles[0].title, "New");
        assert_eq!(feed.articles[1].title, "Old");
        assert!(feed.configured);
    }

    #[test]
    fn bad_entries_are_dropped_and_good_ones_kept() {
        let feed = parse(vec![
            article("Good", "2026-01-01", "https://example.invalid/a"),
            article("Plain link", "2026-01-01", "http://example.invalid/b"),
            article("Script link", "2026-01-01", "javascript:alert(1)"),
            article("", "2026-01-01", "https://example.invalid/c"),
            article("Bad date", "soon", "https://example.invalid/d"),
            article(
                &"x".repeat(MAX_TITLE_CHARS + 1),
                "2026-01-01",
                "https://example.invalid/e",
            ),
            serde_json::json!("not an object"),
        ]);
        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "Good");
    }

    #[test]
    fn repeated_links_appear_once_and_the_list_is_capped() {
        let repeated = (0..MAX_ARTICLES + 5)
            .map(|i| {
                article(
                    &format!("Article {i}"),
                    "2026-01-01",
                    &format!("https://example.invalid/{i}"),
                )
            })
            .chain([article(
                "Again",
                "2026-01-01",
                "https://example.invalid/0",
            )])
            .collect();
        let feed = parse(repeated);
        assert_eq!(feed.articles.len(), MAX_ARTICLES);
        let links: HashSet<_> = feed.articles.iter().map(|a| &a.link).collect();
        assert_eq!(links.len(), MAX_ARTICLES);
    }

    #[test]
    fn categories_are_read_and_unknown_ones_are_general() {
        let raw = serde_json::json!({ "articles": [
            { "title": "A", "date": "2026-01-02", "link": "https://example.invalid/a", "category": "release" },
            { "title": "B", "date": "2026-01-01", "link": "https://example.invalid/b", "category": "something-new" },
            { "title": "C", "date": "2025-01-01", "link": "https://example.invalid/c" },
        ]});
        let feed = parse_feed(raw.to_string().as_bytes()).unwrap();
        assert_eq!(feed.articles[0].category, NewsCategory::Release);
        assert_eq!(feed.articles[1].category, NewsCategory::Announcement);
        assert_eq!(feed.articles[2].category, NewsCategory::Announcement);
    }

    #[test]
    fn only_secure_images_and_links_are_kept() {
        let raw = serde_json::json!({
            "all_news_url": "http://example.invalid/all",
            "articles": [{
                "title": "A", "date": "2026-01-01", "link": "https://example.invalid/a",
                "thumbnail": "http://example.invalid/a.png"
            }]
        });
        let feed = parse_feed(raw.to_string().as_bytes()).unwrap();
        assert_eq!(feed.articles[0].thumbnail, None);
        assert_eq!(feed.all_news_url, None);
    }

    #[test]
    fn a_broken_file_is_an_error_not_an_empty_feed() {
        assert!(parse_feed(b"<html>not json</html>").is_err());
    }

    #[test]
    fn an_unpublished_feed_has_no_news_yet() {
        let feed = unpublished_feed();
        assert!(feed.configured);
        assert!(feed.articles.is_empty());
    }

    #[test]
    fn no_feed_means_not_configured() {
        assert!(!NewsFeed::default().configured);
    }
}
