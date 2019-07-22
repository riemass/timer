use std::{
    collections::BinaryHeap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
        Condvar,
        Mutex,
    },
    time::{Duration, SystemTime},
};

use std::cmp::{Ord, PartialEq, PartialOrd};

#[derive(Eq)]
struct Job {
    time: SystemTime,
    id: i32,
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time).reverse()
    }
}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

struct TimerInner {
    jobs: Mutex<BinaryHeap<Job>>,
    stop: AtomicBool,
    condvar: Condvar,
}

struct Timer {
    inner: Arc<TimerInner>,
    thread: std::thread::JoinHandle<()>,
}

impl Timer {
    pub fn new() -> Self {
        let inner = Arc::new(TimerInner {
            jobs: Mutex::new(BinaryHeap::new()),
            stop: AtomicBool::new(false),
            condvar: Condvar::new(),
        });
        let thread = {
            let inner = inner.clone();
            std::thread::spawn(move || {
                Self::run(inner);
            })
        };

        Self { inner, thread }
    }

    pub fn add(&mut self, job: Job) {
        let mut l = self.inner.jobs.lock().unwrap();
        l.push(job);
        self.inner.condvar.notify_one();
    }

    fn run(inner: Arc<TimerInner>) {
        loop {
            let mut jobs_lock = inner.jobs.lock().unwrap();

            while jobs_lock.len() == 0 && !inner.stop.load(Ordering::Relaxed) {
                jobs_lock = inner.condvar.wait(jobs_lock).unwrap();
            }
            if inner.stop.load(Ordering::Relaxed) {
                return;
            }
            let mut now = SystemTime::now();
            let mut next_timestamp = jobs_lock.peek().unwrap().time;

            while next_timestamp > now {
                let wait = next_timestamp.duration_since(now).unwrap();
                jobs_lock = inner.condvar.wait_timeout(jobs_lock, wait).unwrap().0;
                if inner.stop.load(Ordering::Relaxed) {
                    return;
                }
                next_timestamp = jobs_lock.peek().unwrap().time;
                now = SystemTime::now();
            }
            let job = jobs_lock.pop().unwrap();
            println!("Hello from {} at {:?}", job.id, SystemTime::now());
        }
    }

    pub fn stop(self) {
        self.inner.stop.store(true, Ordering::SeqCst);
        self.inner.condvar.notify_one();
        self.thread.join().unwrap();
    }
}

fn main() {
    let mut timer = Timer::new();
    let now = SystemTime::now();
    std::thread::sleep(Duration::from_millis(100));

    let job = Job {
        time: now - Duration::from_secs(1),
        id: 1,
    };
    timer.add(job);

    let job = Job {
        time: now + Duration::from_secs(3),
        id: 2,
    };
    timer.add(job);

    let job = Job {
        time: now + Duration::from_secs(1),
        id: 3,
    };
    timer.add(job);

    println!("Hello, world! Timer system starting at {:?}", now);
    std::thread::sleep(Duration::from_secs(5));
    timer.stop();
}
