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

struct TaskOnce {
    task: Box<dyn FnOnce() + Send>,
}

struct TaskRepeat {
    task: Box<dyn FnMut() + Send>,
    repeat: Duration,
}

enum Task {
    Once(TaskOnce),
    Repeat(TaskRepeat),
}

impl Task {
    pub fn new_once<T>(task: T) -> Task
    where T: FnOnce() + Send + 'static {
        Task::Once(TaskOnce {
            task: Box::new(task),
        })
    }

    pub fn new_repeat<T>(task: T, repeat: Duration) -> Task
    where T: FnMut() + Send + 'static {
        Task::Repeat(TaskRepeat {
            task: Box::new(task),
            repeat,
        })
    }
}

struct Job {
    time: SystemTime,
    task: Task,
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

impl Eq for Job {}

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

    pub fn schedule_once<J>(&mut self, time: SystemTime, job: J)
    where J: FnOnce() + Send + 'static {
        let job = Job {
            time,
            task: Task::new_once(job),
        };
        let mut l = self.inner.jobs.lock().unwrap();
        l.push(job);
        self.inner.condvar.notify_one();
    }

    pub fn schedule_repeat<J>(&mut self, time: SystemTime, job: J, repeat: Duration)
    where J: FnMut() + Send + 'static {
        let job = Job {
            time,
            task: Task::new_repeat(job, repeat),
        };
        let mut l = self.inner.jobs.lock().unwrap();
        l.push(job);
        self.inner.condvar.notify_one();
    }

    fn run(inner: Arc<TimerInner>) {
        loop {
            let mut job = {
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
                jobs_lock.pop().unwrap()
            };
            match job.task {
                Task::Once(once_task) => {
                    (once_task.task)();
                },
                Task::Repeat(ref mut repeat_task) => {
                    (repeat_task.task)();
                    job.time += repeat_task.repeat;
                    let mut l = inner.jobs.lock().unwrap();
                    l.push(job);
                },
            }
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

    timer.schedule_once(now - Duration::from_secs(1), || {
        println!("Hello1");
    });

    timer.schedule_once(now + Duration::from_secs(3), || {
        println!("Hello2");
    });

    timer.schedule_once(now + Duration::from_secs(1), || {
        println!("Hello3");
    });

    timer.schedule_repeat(
        now + Duration::from_secs(1),
        || {
            println!("Repeat Hi");
        },
        Duration::from_millis(300),
    );

    println!("Hello, world! Timer system starting at {:?}", now);
    std::thread::sleep(Duration::from_secs(5));
    timer.stop();
}
