use std::{collections::BTreeMap, path::PathBuf};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use crate::hn::Item;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Decision { Spam, Upvote, Comment }

impl Decision {
    pub fn label(self) -> &'static str {
        match self { Self::Spam => "Spam", Self::Upvote => "Upvote", Self::Comment => "Upvote + comment" }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter { #[default] Inbox, Upvote, Comment, Spam, All }
impl Filter {
    pub const ALL: [Self; 5] = [Self::Inbox, Self::Upvote, Self::Comment, Self::Spam, Self::All];
    pub fn label(self) -> &'static str {
        match self { Self::Inbox => "Inbox", Self::Upvote => "Upvote", Self::Comment => "Upvote + comment", Self::Spam => "Spam", Self::All => "All posts" }
    }
    pub fn matches(self, decision: Option<Decision>) -> bool {
        match self { Self::Inbox => decision.is_none(), Self::All => true, Self::Upvote => decision == Some(Decision::Upvote), Self::Comment => decision == Some(Decision::Comment), Self::Spam => decision == Some(Decision::Spam) }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Store {
    pub posts: BTreeMap<u64, Item>,
    pub decisions: BTreeMap<u64, Decision>,
    pub drafts: BTreeMap<u64, String>,
    pub synced_at: Option<u64>,
}
impl Store {
    pub fn merge(&mut self, posts: Vec<Item>) {
        for post in posts.into_iter().filter(Item::visible_post) { self.posts.insert(post.id, post); }
        // Keep all reviewed posts and drafts, and the latest 500 other posts.
        let mut unreviewed: Vec<_> = self.posts.values().filter(|p| !self.decisions.contains_key(&p.id) && !self.drafts.contains_key(&p.id)).map(|p| (p.time, p.id)).collect();
        unreviewed.sort_unstable_by(|a, b| b.cmp(a));
        for (_, id) in unreviewed.into_iter().skip(500) { self.posts.remove(&id); }
    }
    pub fn visible(&self, filter: Filter, query: &str) -> Vec<u64> {
        let query = query.trim().to_lowercase();
        let mut posts: Vec<_> = self.posts.values().filter(|post| {
            filter.matches(self.decisions.get(&post.id).copied()) && (query.is_empty() || format!("{} {} {}", post.title(), post.by, post.domain()).to_lowercase().contains(&query))
        }).collect();
        posts.sort_by_key(|post| std::cmp::Reverse((post.time, post.id)));
        posts.iter().map(|post| post.id).collect()
    }
    pub fn count(&self, filter: Filter) -> usize {
        self.posts.keys().filter(|id| filter.matches(self.decisions.get(id).copied())).count()
    }
    pub fn load(path: &std::path::Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("Could not read saved reviews. The existing file has been preserved."),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => bail!("Could not read saved reviews. Check access to the application data folder."),
        }
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec(self)?)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }
}

pub fn data_path() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("XDG_DATA_HOME") { return Some(PathBuf::from(base).join("ruie-review/reviews.json")); }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let base = if cfg!(target_os = "macos") { home.join("Library/Application Support") } else { home.join(".local/share") };
    Some(base.join("ruie-review/reviews.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn post(id: u64, time: u64) -> Item { Item { id, time, title: format!("Post {id}"), kind: "story".into(), ..Default::default() } }
    #[test]
    fn queues_are_exclusive_and_sort_newest_first() {
        let mut store = Store::default();
        store.merge(vec![post(1, 10), post(2, 20), post(3, 30), post(4, 40)]);
        store.decisions.insert(1, Decision::Spam);
        store.decisions.insert(2, Decision::Upvote);
        store.decisions.insert(3, Decision::Comment);
        assert_eq!(store.visible(Filter::Inbox, ""), vec![4]);
        assert_eq!(store.visible(Filter::Comment, ""), vec![3]);
        assert_eq!(store.visible(Filter::All, ""), vec![4, 3, 2, 1]);
        assert_eq!(store.visible(Filter::All, "Post 2"), vec![2]);
        store.decisions.remove(&2);
        assert_eq!(store.visible(Filter::Inbox, ""), vec![4, 2]);
    }
    #[test]
    fn refresh_preserves_reviews_and_filters_dead_items() {
        let mut store = Store::default();
        store.decisions.insert(1, Decision::Comment);
        store.drafts.insert(1, "A useful reply".into());
        let mut dead = post(3, 30); dead.dead = true;
        store.merge(vec![post(1, 10), post(2, 20), dead]);
        store.merge(vec![post(2, 21)]);
        assert_eq!(store.count(Filter::Comment), 1);
        assert_eq!(store.drafts[&1], "A useful reply");
        assert!(!store.posts.contains_key(&3));
        let restored: Store = serde_json::from_slice(&serde_json::to_vec(&store).unwrap()).unwrap();
        assert_eq!(restored.decisions[&1], Decision::Comment);
        assert_eq!(restored.drafts[&1], "A useful reply");
    }
    #[test]
    fn malformed_saved_data_is_not_silently_reset() {
        let path = std::env::temp_dir().join(format!("ruie-corrupt-{}.json", std::process::id()));
        std::fs::write(&path, "broken json").unwrap();
        assert!(Store::load(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken json");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn unsafe_article_links_are_not_opened() {
        let mut item = post(1, 1);
        item.url = "javascript:alert(1)".into();
        assert!(item.article_url().is_none());
        item.url = "https://example.com/article".into();
        assert!(item.article_url().is_some());
    }
}
