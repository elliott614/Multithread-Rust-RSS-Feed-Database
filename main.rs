#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::env;
use std::io;
use std::sync::{Arc, Mutex};

mod common;
mod multi;
mod pooled;
mod single;
mod threadpool;

const MAX_MATCHES: usize = 10;

fn build_single(filename: &str) -> common::RssIndexResult<common::RssIndex> {
    let mut article_index = common::ArticleIndex::new();
    let mut rss_index = common::RssIndex::new();

    single::process_feed_file(filename, &mut article_index)?;
    common::build_index(&mut article_index, &mut rss_index);

    Result::Ok(rss_index)
}

fn build_multi(filename: &str) -> common::RssIndexResult<common::RssIndex> {
    let article_index = Arc::new(Mutex::new(common::ArticleIndex::new()));
    let mut rss_index = common::RssIndex::new();

    multi::process_feed_file(filename, article_index.clone())?;

    let mut final_index = article_index.lock().unwrap();

    common::build_index(&mut final_index, &mut rss_index);

    Result::Ok(rss_index)
}

fn build_pooled(filename: &str) -> common::RssIndexResult<common::RssIndex> {
    let article_index = Arc::new(Mutex::new(common::ArticleIndex::new()));
    let mut rss_index = common::RssIndex::new();

    pooled::process_feed_file(filename, article_index.clone())?;

    let mut final_index = article_index.lock().unwrap();

    common::build_index(&mut final_index, &mut rss_index);

    Result::Ok(rss_index)
}

fn main() -> common::RssIndexResult<()> {
    let mut args = env::args().skip(1);

    let rss_index = match (args.next(), args.next().as_ref().map(String::as_str)) {
        (Some(f), Some("single")) => build_single(&f)?,
        (Some(f), Some("multi")) => build_multi(&f)?,
        (Some(f), Some("pool")) => build_pooled(&f)?,
        _ => {
            println!("Usage: cargo run <filename.xml> [single|multi|pool]");
            return Result::Err(Box::new(common::RssIndexError::ArgsError));
        }
    };

    println!("Done building index.");

    let mut buffer = String::new();
    let mut lower_buffer;

    loop {
        println!("Enter a search term [or just hit <enter> to quit]: ");
        buffer.clear();
        io::stdin().read_line(&mut buffer)?;
        buffer = buffer.trim().to_string();
        if buffer.is_empty() {
            return Result::Ok(());
        }
        lower_buffer = buffer.to_lowercase();
        let matches = rss_index.index.get(&lower_buffer);
        match matches {
            None => println!("We didn't find any matches for \"{}\". Try again.", buffer),
            Some(m) => {
                println!("That term appears in {} articles.", m.len());
                let mut articles = m.iter().collect::<Vec<(&common::Article, &u32)>>();
                articles.sort_by(|art1, art2| {
                    // sort by decreasing hits and alphabetical title
                    art2.1.cmp(&art1.1).then(art1.0.title.cmp(&art2.0.title))
                });
                if articles.len() > MAX_MATCHES {
                    println!("Here are the top {} of them:", MAX_MATCHES);
                } else {
                    println!("Here they are:");
                }
                for (art, count) in articles
                    .iter()
                    .take(std::cmp::min(articles.len(), MAX_MATCHES))
                {
                    let times = if **count == 1 { "time" } else { "times" };
                    println!("\"{}\" [appears {} {}].", art.title, count, times);
                    println!("        \"{}\"", art.url);
                }
            }
        }
    }
}
