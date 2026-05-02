use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::error::ErrorCode;
use hashbrown::HashMap;

use crate::aio::channel::ChannelAio;
use crate::sink::Event;
use crate::sink::Sink;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TaskId(u32);

struct RunQueue(spin::Mutex<VecDeque<TaskId>>);

impl RunQueue {
    const fn new() -> Self {
        Self(spin::Mutex::new(VecDeque::new()))
    }

    fn push(&self, task_id: TaskId) {
        self.0.lock().push_back(task_id);
    }

    fn pop(&self) -> Option<TaskId> {
        self.0.lock().pop_front()
    }
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
    waker: Waker,
}

impl Task {
    fn poll(&mut self) -> Poll<()> {
        let mut ctx = Context::from_waker(&self.waker);
        self.future.as_mut().poll(&mut ctx)
    }
}

struct TaskWaker {
    task_id: TaskId,
    run_queue: Arc<RunQueue>,
}

impl TaskWaker {
    fn new(task_id: TaskId, run_queue: Arc<RunQueue>) -> Self {
        Self { task_id, run_queue }
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.run_queue.push(self.task_id);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.run_queue.push(self.task_id);
    }
}

pub(super) struct Executor {
    next_task_id: AtomicU32,
    tasks: spin::Mutex<HashMap<TaskId, Task>>,
    pub(super) channels: ChannelAio,
    run_queue: Arc<RunQueue>,
    sink: Sink,
}

impl Executor {
    pub fn new() -> Result<Self, ErrorCode> {
        let sink = Sink::new()?;
        Ok(Self {
            next_task_id: AtomicU32::new(0),
            tasks: spin::Mutex::new(HashMap::new()),
            channels: ChannelAio::new(),
            run_queue: Arc::new(RunQueue::new()),
            sink,
        })
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + Send + Sync + 'static) {
        let mut tasks = self.tasks.lock();
        let task_id = TaskId(self.next_task_id.fetch_add(1, Ordering::Relaxed));
        let waker = TaskWaker::new(task_id, self.run_queue.clone());
        let task = Task {
            future: Box::pin(future),
            waker: Waker::from(Arc::new(waker)),
        };
        tasks.insert(task_id, task);
        self.run_queue.push(task_id);
    }

    fn run_runnable_tasks(&self) {
        while let Some(task_id) = self.run_queue.pop() {
            let mut task = match self.tasks.lock().remove(&task_id) {
                Some(task) => task,
                None => {
                    // Wakers may enqueue the same task multiple times. Ignore the
                    // duplicate wakeups.
                    continue;
                }
            };

            match task.poll() {
                Poll::Ready(()) => {
                    // Task completed.
                }
                Poll::Pending => {
                    // Task has reached an await point. Re-insert it back into
                    // the map.
                    self.tasks.lock().insert(task_id, task);
                }
            }
        }
    }

    pub fn run(&self) {
        loop {
            self.run_runnable_tasks();
            if self.tasks.lock().is_empty() {
                // No tasks to run. Exit the loop.
                return;
            }

            let (id, event) = self.sink.wait().unwrap();
            match event {
                Event::Message(peek) => {
                    self.channels.handle_message(id, peek);
                }
                Event::PeerClosed => {
                    todo!()
                }
                Event::Irq { irq } => {
                    todo!()
                }
                Event::Timer => {
                    todo!()
                }
            }
        }
    }
}

pub(super) static GLOBAL_EXECUTOR: spin::Lazy<Executor> =
    spin::Lazy::new(|| Executor::new().unwrap());

pub fn spawn(future: impl Future<Output = ()> + Send + Sync + 'static) {
    GLOBAL_EXECUTOR.spawn(future);
}

pub fn run(future: impl Future<Output = ()> + Send + Sync + 'static) {
    spawn(future);
    GLOBAL_EXECUTOR.run();
}
