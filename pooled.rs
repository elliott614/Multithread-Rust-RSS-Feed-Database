use rss::Channel;
use std::fs::File;
use std::io::BufReader;
use std::result::Result;

use std::sync::{Arc, Mutex};
use url::Url;

use crate::common::*;
use crate::threadpool::*;

/// Thread pool sizes.
const SIZE_FEEDS_POOL: usize = 3;
const SIZE_SITES_POOL: usize = 20;

/// Same as the single/multi threaded version, but using a thread pool. Set up two thread pools:
/// one for handling feeds, and one for handling articles. Use the sizes above. Push closures
/// executing `process_feed` into the thread pool.
pub fn process_feed_file(file_name: &str, index: Arc<Mutex<ArticleIndex>>) -> RssIndexResult<()> {
    let mut feed_thread_pool = ThreadPool::new(SIZE_FEEDS_POOL);
    let mut sites_thread_pool = ThreadPool::new(SIZE_SITES_POOL);
    let stp_arc = Arc::new(Mutex::new(sites_thread_pool));
    let file = File::open(file_name)?;
    println!("Processing feed file: {}", file_name);
    let channel = Channel::read_from(BufReader::new(file))?;
    for feed in channel.into_items() {
        let ind = Arc::clone(&index);
        let stp_arc2 = Arc::clone(&stp_arc);
        let pff = move || {
            let url = feed.link().ok_or(RssIndexError::UrlError).unwrap();
            let title = feed.title().ok_or(RssIndexError::UrlError).unwrap();
            println!("Processing feed: {} [{}]", title, url);
            process_feed(url, ind, stp_arc2).unwrap();
        };
        feed_thread_pool.execute(pff);
    }
    Result::Ok(())
}

/// Same as the single/multi threaded version, but using a thread pool. Push closures executing
/// `process_article` into the thread pool that is passed in.
fn process_feed(
    url: &str,
    index: Arc<Mutex<ArticleIndex>>,
    sites_pool: Arc<Mutex<ThreadPool>>,
) -> RssIndexResult<()> {
    let channel = Channel::from_url(url)?;
    let items = channel.into_items();
    for item in items {
        let ind = Arc::clone(&index);
        let (url, site, title) = match (
            item.link(),
            Url::parse(&url).unwrap().host_str(),
            item.title(),
        ) {
            (Some(u), Some(s), Some(t)) => (u.to_string(), s.to_string(), t.to_string()),
            _ => continue,
        };
        let pf = move || {
            println!("Processing article: {} [{}]", title, url);
            let article = Article::new(url.to_string(), title.to_string());
            let article_words = process_article(&article).unwrap();
            ind.lock().unwrap().add(
                site.to_string(),
                title.to_string(),
                url.to_string(),
                article_words,
            );
        };
        sites_pool.lock().unwrap().execute(pf);
    }
    Result::Ok(())
}
