//! Async I/O support (`async fn`).

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use hashbrown::HashMap;

use crate::channel::Channel;
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
    future: Pin<Box<dyn Future<Output = ()>>>,
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

    fn do_wake(&self) {
        self.run_queue.push(self.task_id);
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.do_wake();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.do_wake();
    }
}

#[derive(Debug)]
enum Error {
    Sink(ErrorCode),
}

struct Executor {
    tasks: HashMap<TaskId, Task>,
    run_queue: Arc<RunQueue>,
    sink: Sink,
}

impl Executor {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::Sink)?;
        Ok(Self {
            tasks: HashMap::new(),
            run_queue: Arc::new(RunQueue::new()),
            sink,
        })
    }

    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) {
        let task_id = TaskId(self.tasks.len() as u32);
        let waker = TaskWaker::new(task_id, self.run_queue.clone());
        let task = Task {
            future: Box::pin(future),
            waker: Waker::from(Arc::new(waker)),
        };
        self.tasks.insert(task_id, task);
        self.run_queue.push(task_id);
    }

    fn run_runnable_tasks(&mut self) {
        while let Some(task_id) = self.run_queue.pop() {
            let task = self.tasks.get_mut(&task_id).unwrap();
            match task.poll() {
                Poll::Ready(()) => {
                    // Task completed.
                    self.tasks.remove(&task_id);
                }
                Poll::Pending => {
                    // Task has reached an await point. It will register itself
                    // later with the waker so we don't need to do anything here.
                }
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            self.run_runnable_tasks();
        }
    }
}
