//! Running hub use cases off the frame thread.
//!
//! Status reads go to a small fixed pool so that refreshing fifty
//! repositories does not start fifty `git` processes at once. User-started
//! operations (clone, pull, install) each get their own thread: they are few,
//! they may run for minutes, and they must not queue behind each other.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

type Task = Box<dyn FnOnce() + Send + 'static>;

pub struct Pool {
    sender: Sender<Task>,
}

impl Pool {
    pub fn new(workers: usize) -> Self {
        let (sender, receiver) = channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..workers {
            let receiver = Arc::clone(&receiver);
            std::thread::spawn(move || worker(receiver));
        }
        Self { sender }
    }

    pub fn submit(&self, task: impl FnOnce() + Send + 'static) {
        // Fails only once every worker has exited, i.e. at shutdown.
        let _ = self.sender.send(Box::new(task));
    }
}

fn worker(receiver: Arc<Mutex<Receiver<Task>>>) {
    loop {
        let task = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        match task {
            Ok(task) => task(),
            Err(_) => return,
        }
    }
}

pub fn spawn(task: impl FnOnce() + Send + 'static) {
    std::thread::spawn(task);
}
