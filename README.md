# Concurrent RSS Indexer

In this assignment, you will take an indexer for RSS feeds and make it
multithreaded. An RSS file is an XML file (see feeds/ for two examples), in
which each item points to an RSS feed somewhere on the internet. Each RSS feed
is then another XML file containing a list of items (say, news articles), each
with a title and a URL. Your indexer will download each of these articles and
build a big map of word counts, letting you query the articles for where a
given word appears the most frequently.

## Step 0: Read and run the single-threaded version (common.rs, single.rs)

The first step is to look over the single-threaded indexer, which we've given
you split between two files. The common functionality is in `common.rs`, and the
single-threaded processing functions are in `single.rs`. In the next two steps,
you will re-implement the two functions in `single.rs` so that they use threads.
Try out the indexer on the file `feeds/small-feed.xml` by running `cargo run
feeds/small-feed.xml single`. It's going to take a while... If have lots of
time on your hands, you can try it on `feeds/medium-feed.xml`.

## Step 1: Multi-threaded version (multi.rs)

Now, you're going to make the indexer multithreaded.  This will be implemented
in multi.rs. For each feed in the feed file, you should spawn a new thread. For
each article in each feed, you should spawn a new thread to process the article
(this is all the stuff in `process_article`: fetching the article, scraping it,
and indexing it).

You'll use several concurrency primitives that we've seen in class:

* `Arc`: share something between multiple threads in a thread-safe manner
* `Mutex`: mutual-exclusion locks.
* `Condvar`condition variables.

See the documentation in `std::sync` for examples about how to use these
primitives. To build the index in a thread-safe way, we've set up some
datastructures in multi.rs for managing Mutexes and Condvars.  You're welcome to
change them, but you shouldn't have to.

Note that calling wait on a Condvars will release exactly one lock: the one you
pass to wait. If your program is holding any other locks at this time, they will
*not* be automatically released. Watch out for deadlocks here...

Your multi-threaded version should respect some resource limits:

* At any time, there should be at most `MAX_THREADS_FEEDS` feed threads.
* At any time, there should be at most `MAX_THREADS_SITES` article threads per
  hostname. This limits the number of requests we're making to any one site.
* At any time, there should be at most `MAX_THREADS_TOTAL` threads in total.

We've set up a struct `ThreadCount` to keep track of these counts. Each field
combines a Mutex and Condvar for waiting/signaling this field. You will have to
update these counts (incrementing and decrementing) as threads are
spawned/finished.

* `feeds_count`: count of how many feeds threads there are
* `sites_count`: count of how many article threads there are using a HashMap,
  the key is the hostname (called `site` in `process_feed`)
* `total_count`: count of total number of threads

You should reuse stuff where possible, i.e., everything in `common.rs` should
continue to work. Try out your multi-threaded version---should be much faster!

## Step 2: Thread-pool version, with thread pool (pooled.rs, threadpool.rs)

Next, we'll build an indexer backed by a *thread pool*. A thread pool allows a
client program to farm out jobs (closures from () to ()) to a pool of *worker*
threads. We've provided the skeleton of a thread pool in threadpool.rs. Fill in
the implementation there.  The size of the pool will be fixed at initialization.
When the pool is dropped, we should first signal all workers to finish, and then
wait for all worker threads (via `join`).

You'll be using channels from `std::sync::mpsc` for sending jobs to workers.
Keep in mind that `mpsc` stands for "multiple producer, single consumer" but
we're using it as a single producer (dispatching jobs) to multiple consumers
(workers receiving jobs), so you'll have to wrap the receiving end in an Arc and
Mutex to share it safely between the workers.

For annoying technical reasons, your closures will have type FnBox instead of
FnOnce. Roughly, FnOnce closures must be placed on the stack before they can be
called, which means they must have a size known at compile time. The jobs we're
sending will have varying sizes, so they will be stored in Boxes on the heap.
The FnBox trait in `common.rs` lets us call these closures without putting them
on to the stack. We've implemented the traits, so things should work for regular
closures. To call a FnBox functions, use the method `call_box`, like so:
`my_boxed_fn.call_box()`.

Once you've got the thread pool working, set up your indexer to use the thread
pool. You should use two pools: the feeds pool should have three workers
handling all feed fetching and processing, while the sites pool should have
twenty workers total handling all article fetching and processing. You don't
need to respect any other limits for this version of the indexer.

With the thread pool backing the indexer, you shouldn't have to use much
synchronization in this version (though you'll still have to Arc and Mutex the
pools themselves, to share them between threads). Instead, just dump jobs into
the pool. Test out your thread pool backed implementation. It should be about as
fast as the multi-threaded version.

## Hard requirements

All version of your code should respect a few basic properties:

* Never download the same URL twice. In fact, never even *attempt* to download
  the same URL twice.
* If two articles have the exact same titles and hostnames but different URLs,
  still fetch and process the second one. Using `ArticleIndex::add` will
  intersect the words in the two articles to reduce dynamic content (like ads)
  while recording all of the URLs.

## Error handling

* You should not use `?` inside your threads.  You can usually use the `?`
  syntax to propagate errors when the enclosing function returns a `Result`
  type, but it's a bit troublesome to have threads return things of type
  `Result`. Instead, have your threads return things of type `()` (nothing). If
  you encounter an error while processing articles/feeds (getting back
  `Err(...)` after calling `process_article` or `process_feed`), you should
  silently ignore the error.
* You can either use an `if let Ok(...) = ...` to only handle the `Ok` case, or
  you can look into `Result::unwrap_or_default`. 
* Do not panic if you get back an `Err` (don't use regular `unwrap`).
* We've set up a custom type for errors. We've added three kinds of errors.
  `UrlError` can happen when the URL has missing fields (no link, parsing
  broken, no hostname, etc.). `ScraperError` is some kind of error with scraper
  (unable to find body tag, parse body information, etc.). You shouldn't have to
  deal with `ArgsError`, which reports errors from command line arguments.
* If you're throwing new errors, you'll have to put the error in a Box::new().
* Keep your use of `unwrap` to an absolute minimum. It's fine for initial
  prototyping, but we expect you to remove as many instances as you can.  The
  only places where `unwrap` can be tolerated are for `lock()` on Mutex, and for
  `join()` on threads. Even there, it's good practice to use `expect` instead to
  add on a string describing the error before panic.
* You should not have to do anything special for errors resulting from external
  crates like rss/scraper/reqwest---`?` should propagate the error just fine.

## Collections

This assignment uses more parts of Rust's std::collections library.  You'll want
to read more about `HashMap` and `HashSet` in the collections documentation.
Focus especially on the [Entry API](https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.entry),
a pattern used in the Rust libraries to add-and-get-ref to an element in a
collection with a single lookup.

## External crates

Rust has a huge collection of external crates, which we'll be leveraging for
this assignment. Dependencies are usually listed in Cargo.toml (take a look,
it's very simple). Cargo handles everything else: downloading, building,
updating, etc. We've stocked your Cargo.toml with a few crates:

* [rss](https://crates.rs/crates/rss). Crate for working with RSS feeds. Note
  that we included the `from_url` feature for this crate in your Cargo.toml, so
  `from_url` should work.
* [reqwest](https://crates.rs/crates/reqwest). A high-level HTTP crate. You'll
  use this for grabbing data from web pages, though it can do a lot more.
* [scraper](https://crates.rs/crates/scraper). A crate for parsing and querying
  HTML, built on the lower-level crate `html5ever` used by Mozilla's Servo
  project. You'll use this for parsing HTML and querying tags (e.g., grabbing
  the contents of the "<body>" tag)
* [url](https://crates.rs/crates/url). A crate for working with URLs: parsing,
  getting hostname, etc.

You can also see these crates on [crates.io](crates.io). The crates all have
good documentation with examples of how to use the different functions ("API
documentation"), you'll want to keep these references handy. Look at the
examples to get started.

## Extensions

These extensions will explore some other Cargo packages. Again, they're probably
not worth the point value---you should only do these if you're interested. The
extension should be completely independent, you can do whichever ones you find
interesting. You'll have to add these packages to your Cargo.toml.

### Adding a stemmer (1)

In many languages, a single base word can appear in many different forms. For
instance, the word "bake" appears also as "baked", "bakes", "baking", "baker",
etc. Many text indexers first apply a *stemmer* to cut down a word into a core
"stem", so that seemingly different words will be treated as one group.

Modify the article processing code to stem words before counting them. Then,
tweak the query code so that user queries are stemmed before lookup. There are
many Rust crates available for stemming; rust-stemmers handles a variety of
languages:

https://crates.rs/crates/rust-stemmers

### Crossbeam channels (1)

Crossbeam is a collection of high quality concurrency primitives for Rust. Take
a look at the docs here:

https://docs.rs/crate/crossbeam/0.7.1

Crossbeam has a better channel implementation than the built-in std::sync::mpsc.
Crossbeam channels support multiple consumers and have other useful features
like selecting between multiple channels. Replace std::sync::mpsc in your thread
pool with crossbeam::channel. You should be able to remove a few Arcs/Mutexes.

### Crossbeam scoped threads (2)

Crossbeam scopes (crossbeam::scope) allows making a scope for new threads. The
idea is that the threads in the scope can borrow data from the external scope,
which is guaranteed to be valid since all threads in the scope will be joined
when the scope is dropped. Adjust your implementation of multi to use scoped
threads.  You should be able to cut out a whole bunch of clones and Arcs, though
you'll probably still need Mutexes.

### Crossbeam ArrayDeque (2)

Crossbeam supports various *lockless*, or *non-blocking* datastructures. These
datastructures are thread safe and guaranteed not to block when they are
accessed. The non-blocking queues in Crossbeam allow multiple threads to push or
pop from a shared queue without locking. Implement a new threand pool based on
crossbeam::queue::ArrayQueue or crossbeam::queue::SegQueue instead of channels.

### Crossbeam deque (3)

Some facilities in crossbeam are designed especially for implementing thread
pools. *Work-stealing* parallelism is a fancier way to implement thread pools,
where each thread maintains a queue of jobs to process. If a thread finishes its
queue early, it looks at queues from other threads and "steals" jobs from them.
In this way, the thread pool tries to automatically keep all threads fed with
jobs. Implement a new, work-stealing thread pool using the primitives in
crossbeam::deque.
