//! The official, public, read-only Hacker News Firebase API.
use std::{sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};

use anyhow::{Result, bail};
use futures::{AsyncReadExt, StreamExt, stream};
use gpui_kit::http_client::{AsyncBody, HttpClient};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const API: &str = "https://hacker-news.firebaseio.com/v0";
pub const PAGE_SIZE: usize = 50;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub by: String,
    pub time: u64,
    pub url: String,
    pub text: String,
    pub score: u64,
    pub descendants: u64,
    pub kids: Vec<u64>,
    pub deleted: bool,
    pub dead: bool,
    #[serde(rename = "type")]
    pub kind: String,
}

impl Item {
    pub fn visible_post(&self) -> bool {
        self.id > 0 && !self.deleted && !self.dead && matches!(self.kind.as_str(), "story" | "job" | "poll")
    }

    pub fn title(&self) -> String {
        html_escape::decode_html_entities(&self.title).into_owned()
    }

    pub fn discussion_url(&self) -> String {
        format!("https://news.ycombinator.com/item?id={}", self.id)
    }

    pub fn article_url(&self) -> Option<String> {
        let url = url::Url::parse(&self.url).ok()?;
        matches!(url.scheme(), "https" | "http").then(|| url.to_string())
    }

    pub fn domain(&self) -> String {
        url::Url::parse(&self.url).ok()
            .and_then(|url| url.host_str().map(|host| host.trim_start_matches("www.").to_owned()))
            .unwrap_or_else(|| "news.ycombinator.com".into())
    }
}

pub async fn get<T: DeserializeOwned>(client: &Arc<dyn HttpClient>, path: &str) -> Result<T> {
    let request = async {
        let mut response = client.get(&format!("{API}/{path}.json"), AsyncBody::empty(), true).await?;
        if !response.status().is_success() { bail!("HN returned {}", response.status()); }
        let mut bytes = Vec::new();
        response.body_mut().take(4 * 1024 * 1024).read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    };
    // Bound every request, including reading the body, without depending on a Tokio runtime.
    let timeout = async {
        futures_timer::Delay::new(Duration::from_secs(20)).await;
        bail!("Hacker News request timed out")
    };
    futures::pin_mut!(request, timeout);
    match futures::future::select(request, timeout).await {
        futures::future::Either::Left((result, _)) | futures::future::Either::Right((result, _)) => result,
    }
}

/// Bounded parallel fetches; retain successful posts if individual items fail.
pub async fn items(client: Arc<dyn HttpClient>, ids: Vec<u64>) -> (Vec<Item>, usize) {
    let results = stream::iter(ids.into_iter().map(|id| {
        let client = client.clone();
        async move { get::<Option<Item>>(&client, &format!("item/{id}")).await }
    })).buffer_unordered(8).collect::<Vec<_>>().await;
    let mut failed = 0;
    let mut items = Vec::new();
    for result in results {
        match result {
            Ok(Some(item)) => items.push(item),
            Ok(None) => {},
            Err(_) => failed += 1,
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse((item.time, item.id)));
    (items, failed)
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn age(time: u64) -> String {
    let seconds = now().saturating_sub(time);
    match seconds {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", seconds / 60),
        3600..86400 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86400),
    }
}
