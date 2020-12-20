use rss::Channel;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::result::Result;

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use crate::common::*;

/// Thread limits.
const MAX_THREADS_FEEDS: u32 = 5;
const MAX_THREADS_SITES: u32 = 10;
const MAX_THREADS_TOTAL: u32 = 18;

/// A lock around some T, with a condition variable for notifying/waiting.
struct CvarLock<T> {
    mutex: Mutex<T>,
    condvar: Condvar,
}

impl<T> CvarLock<T> {
    fn new(data: T) -> Self {
        let mutex = Mutex::new(data);
        let condvar = Condvar::new();
        CvarLock { mutex, condvar }
    }
}

/// Locks/Condvars around counters, tracking the number of feed threads, the number of article
/// threads per site, and the total number of threads.
pub struct ThreadCount {
    feeds_count: CvarLock<u32>,
    sites_count: CvarLock<HashMap<String, u32>>,
    total_count: CvarLock<u32>,
}

/// Same as for the single-threaded version, but now spawn a new thread for each call to
/// `process_feed`. Make sure to respect the thread limits!
pub fn process_feed_file(file_name: &str, index: Arc<Mutex<ArticleIndex>>) -> RssIndexResult<()> {
    let thread_count = Arc::new(ThreadCount {
        feeds_count: CvarLock::new(0),
        sites_count: CvarLock::new(HashMap::new()), //default u32 = 0
        total_count: CvarLock::new(0),
    });
    let mut handles = vec![];
    let file = File::open(file_name)?;
    println!("Processing feed file: {}", file_name);
    let channel = Channel::read_from(BufReader::new(file))?;
    for feed in channel.into_items() {
        let ind = Arc::clone(&index);
        let tc = Arc::clone(&thread_count);
        let handle = thread::spawn(move || {
            let fc_cvl = &tc.feeds_count;
            let fc_cvar = &fc_cvl.condvar;
            let tc_cvl = &tc.total_count;
            let tc_cvar = &tc_cvl.condvar;
            let &ref fc_lock = &fc_cvl.mutex;
            let &ref tc_lock = &tc_cvl.mutex;
            {
                let mut fc = fc_lock.lock().unwrap(); //keeps borrow checker happy to not be moving the value inside the mutex inside self.insert
                while *fc >= MAX_THREADS_FEEDS {
                    fc = fc_cvar.wait(fc).unwrap() //wait for condvar signal
                }
                *fc += 1; //increment feeds_count
                          // println!("feeds count incremented to: {}", *fc);
            }
            {
                let mut tot_c = tc_lock.lock().unwrap();
                while *tot_c >= MAX_THREADS_TOTAL {
                    // println!("sleeping until total lower");
                    tot_c = tc_cvar.wait(tot_c).unwrap() //sleep until condvar signal
                }
                *tot_c += 1; //increment total_count
                             // println!("total count incremented to: {}", *tot_c);
            }
            let url = feed.link().ok_or(RssIndexError::UrlError).unwrap();
            let title = feed.title().ok_or(RssIndexError::UrlError).unwrap();
            println!("Processing feed: {} [{}]", title, url);
            process_feed(url, ind, Arc::clone(&tc)).unwrap();
            {
                let mut fc = fc_lock.lock().unwrap();
                *fc -= 1; //decrement feeds_count
                          // println!("feeds count decremented to: {}", *fc);
                fc_cvar.notify_one(); //signal on condvar
            }
            {
                let mut tot_c = tc_lock.lock().unwrap();
                *tot_c -= 1; //decrement feeds_count
                             // println!("total count decremented to: {}", *tot_c);
                tc_cvar.notify_one();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join();
    }
    Result::Ok(())
}

/// Same as for the single-threaded version, but now spawn a new thread for each call to
/// `process_article`. Make sure to respect the thread limits!
fn process_feed(
    url: &str,
    index: Arc<Mutex<ArticleIndex>>,
    counters: Arc<ThreadCount>,
) -> RssIndexResult<()> {
    let channel = Channel::from_url(url)?;
    let items = channel.into_items();
    let mut handles = vec![];
    for item in items {
        let tc = Arc::clone(&counters);
        let ind = Arc::clone(&index);
        let (url, site, title) = match (
            item.link(),
            Url::parse(&url).unwrap().host_str(),
            item.title(),
        ) {
            (Some(u), Some(s), Some(t)) => (u.to_string(), s.to_string(), t.to_string()),
            _ => continue,
        };
        let mut scount = 0; //keep track of this site count to update sites_count
        let handle = thread::spawn(move || {
            let sc_cvl = &tc.sites_count;
            let tc_cvl = &tc.total_count;
            let sc_cvar = &sc_cvl.condvar;
            let tc_cvar = &tc_cvl.condvar;
            let &ref sc_lock = &sc_cvl.mutex;
            let &ref tc_lock = &tc_cvl.mutex;
            {
                let mut sc = sc_lock.lock().unwrap();
                match sc.get(&site) {
                    Some(val) => {}
                    None => {
                        //clone(), to_string(), String::from(), same thing. don't like seeing "clone()" all over
                        sc.insert(site.to_string(), 0);
                    }
                };
                while *sc.get(&site).unwrap() >= MAX_THREADS_SITES {
                    // println!("sleeping until site: {} lower", site);
                    sc = sc_cvar.wait(sc).unwrap();
                }
                scount = *sc.get(&site).unwrap() + 1;
                sc.insert(site.to_string(), scount); //increment sites_count
                                                     // println!("site: {} count incremented to: {}", site, scount);
            }
            {
                let mut tot_c = tc_lock.lock().unwrap();
                while *tot_c >= MAX_THREADS_TOTAL {
                    // println!("sleeping until total lower");
                    tot_c = tc_cvar.wait(tot_c).unwrap() //sleep until condvar signal
                }
                *tot_c += 1; //increment total_count
                             // println!("total count incremented to: {}", *tot_c);
            }
            println!("Processing article: {} [{}]", title, url);
            let article = Article::new(url.to_string(), title.to_string());
            let article_words = process_article(&article).unwrap();
            ind.lock().unwrap().add(
                site.to_string(),
                title.to_string(),
                url.to_string(),
                article_words,
            );
            {
                let mut sc = sc_lock.lock().unwrap();
                scount = *sc.get(&site).unwrap() - 1;
                sc.insert(site.to_string(), scount); //decrement sites_count
                                                     // println!("site: {} count decremented to: {}", site, scount);
                sc_cvar.notify_one(); //signal a condvar to wake up
            }
            {
                let mut tot_c = tc_lock.lock().unwrap();
                *tot_c -= 1; //decrement feeds_count
                             // println!("total count decremented to: {}", *tot_c);
                tc_cvar.notify_one();
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join();
    }
    Result::Ok(())
}
