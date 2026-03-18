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

use ftl_types::channel::MessageId;
use ftl_types::channel::MessageInfo;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
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
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.run_queue.push(self.task_id);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.run_queue.push(self.task_id);
    }
}

struct InflightMap {
    entries: HashMap<(HandleId, MessageId), Waker>,
    next_mid: u16,
}

impl InflightMap {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_mid: 0,
        }
    }

    pub fn prepare_call(
        &mut self,
        handle_id: HandleId,
        waker: Waker,
    ) -> Result<MessageId, ErrorCode> {
        let first_mid = self.next_mid;
        loop {
            let mid = MessageId::new(self.next_mid);
            self.next_mid = (self.next_mid + 1) & 0xfff;
            let key = (handle_id, mid);
            if self.entries.contains_key(&key) {
                self.entries.insert(key, waker);
                return Ok(mid);
            }

            if self.next_mid == first_mid {
                return Err(ErrorCode::TooManyCalls);
            }
        }
    }

    pub fn complete_call(&mut self, handle_id: HandleId, mid: MessageId) -> Option<Waker> {
        self.entries.remove(&(handle_id, mid))
    }
}

#[derive(Debug)]
enum Error {
    Sink(ErrorCode),
}

struct Executor {
    tasks: HashMap<TaskId, Task>,
    run_queue: Arc<RunQueue>,
    inflights: Arc<spin::Mutex<InflightMap>>,
    sink: Sink,
}

impl Executor {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::Sink)?;
        Ok(Self {
            tasks: HashMap::new(),
            run_queue: Arc::new(RunQueue::new()),
            inflights: Arc::new(spin::Mutex::new(InflightMap::new())),
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
            let (id, event) = self.sink.wait().unwrap();
            match event {
                Event::Message { info, arg } => {
                    //
                }
                Event::PeerClosed => {
                    todo!()
                }
            }
        }
    }
}
