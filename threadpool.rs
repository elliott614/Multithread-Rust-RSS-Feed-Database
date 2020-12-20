use crate::common;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Message type to communicate with workers. A JobMsg is either a closure (FnBox annoyance needed
/// for to get around some limits of Rust closures) or None, which signals the worker to shut down.
type JobMsg = Option<Box<dyn common::FnBox + Send + 'static>>;

/// A ThreadPool should have a sending-end of a mpsc channel (`mpsc::Sender`) and a vector of
/// `JoinHandle`s for the worker threads.
pub struct ThreadPool {
    sender: mpsc::Sender<JobMsg>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPool {
    /// Spin up a thread pool with `num_workers` threads. Workers should all share the same
    /// receiving end of an mpsc channel (`mpsc::Receiver`) with appropriate synchronization. Each
    /// thread should loop and (1) listen for new jobs on the channel, (2) execute received jobs,
    /// and (3) quit the loop if it receives None.
    pub fn new(num_workers: usize) -> Self {
        let (sender, receiver): (mpsc::Sender<JobMsg>, mpsc::Receiver<JobMsg>) = mpsc::channel();
        let mut workers = vec![];
        let rx = Arc::new(Mutex::new(receiver));
        for i in 0..num_workers {
            let rx_i = Arc::clone(&rx);
            let worker = thread::spawn(move || loop {
                //instantiate job here so mutex can unlock before job started
                let mut job: Box<dyn common::FnBox + std::marker::Send> = Box::new(|| ());
                {
                    if let Ok(job_msg) = rx_i.lock().unwrap().recv() {
                        match job_msg {
                            None => break,
                            Some(fn_box) => {
                                job = fn_box;
                            }
                        }
                    }
                }
                job.call_box();
            });
            workers.push(worker);
        }
        ThreadPool { sender, workers }
    }

    /// Push a new job into the thread pool. (You'll probably want to add some constraints.)
    pub fn execute<F>(&mut self, job: F)
    where
        F: common::FnBox + Send + 'static,
    {
        self.sender.send(Some(Box::new(job))).unwrap();
    }
}

impl Drop for ThreadPool {
    /// Clean up the thread pool. Send a kill message (None) to each worker, and join each worker.
    /// This function should only return when all workers have finished.
    fn drop(&mut self) {
        for _ in 0..self.workers.len() {
            self.sender.send(None).unwrap();
        }
        while let Some(worker) = self.workers.pop() {
            worker.join();
        }
    }
}
