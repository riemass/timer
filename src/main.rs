// #[allow(unused_imports)]
// #[allow(dead_code)]
use core::cmp::Reverse;
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

#[derive(Eq, PartialEq, PartialOrd, Ord)]
struct Job {
    time: SystemTime,
    id: i32,
}

struct Timer {
    jobs: Arc<(Mutex<BinaryHeap<Reverse<Job>>>, AtomicBool, Condvar)>,
    thread: std::thread::JoinHandle<()>,
}

impl Timer {
    pub fn new() -> Self {
        let jobs = Arc::new((
            Mutex::new(BinaryHeap::new()),
            AtomicBool::new(false),
            Condvar::new(),
        ));
        let thread = {
            let jobs = jobs.clone();
            std::thread::spawn(move || {
                Self::run(jobs);
            })
        };

        Self { jobs, thread }
    }

    pub fn add(&mut self, job: Job) {
        let mut l = self.jobs.0.lock().unwrap();
        l.push(Reverse(job));
        self.jobs.2.notify_one();
    }

    fn run(jobs: Arc<(Mutex<BinaryHeap<Reverse<Job>>>, AtomicBool, Condvar)>) {
        loop {
            let &(ref jobs, ref stop, ref condvar) = &*jobs;
            let mut jobs_lock = jobs.lock().unwrap();

            while jobs_lock.len() == 0 && !stop.load(Ordering::Relaxed) {
                jobs_lock = condvar.wait(jobs_lock).unwrap();
            }
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let now = SystemTime::now();
            let job = jobs_lock.pop().unwrap().0;
            if job.time > now {
                let (jobs, wait) = condvar
                    .wait_timeout(jobs_lock, job.time.duration_since(now).unwrap())
                    .unwrap();
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                jobs_lock = jobs;
                if !wait.timed_out() {
                    jobs_lock.push(Reverse(job));
                    continue;
                }
            }
            println!("Hello from {} at {:?}", job.id, SystemTime::now());
        }
    }

    pub fn stop(self) {
        self.jobs.1.store(true, Ordering::SeqCst);
        self.jobs.2.notify_one();
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
