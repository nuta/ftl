//! Async I/O support (`async fn`).

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::MessageId;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageKind;
use ftl_types::channel::OpenOptions;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::channel::Channel;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
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

enum Inflight {
    Reserved,
    WaitingForReply(Waker),
    Received {
        info: MessageInfo,
        arg1: usize,
        arg2: usize,
    },
}

struct InflightMap {
    entries: HashMap<(HandleId, MessageId), Inflight>,
    next_mid: u16,
}

impl InflightMap {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_mid: 0,
        }
    }

    pub fn alloc_mid(&mut self, handle_id: HandleId) -> Result<MessageId, ErrorCode> {
        let first_mid = self.next_mid;
        loop {
            let mid = MessageId::new(self.next_mid);
            self.next_mid = (self.next_mid + 1) & 0xfff;

            let key = (handle_id, mid);
            if !self.entries.contains_key(&key) {
                self.entries.insert(key, Inflight::Reserved);
                return Ok(mid);
            }

            if self.next_mid == first_mid {
                return Err(ErrorCode::TooManyCalls);
            }
        }
    }

    fn set_waker(&mut self, handle_id: HandleId, mid: MessageId, waker: Waker) {
        let key = (handle_id, mid);
        debug_assert!(matches!(
            self.entries.get(&key),
            Some(Inflight::Reserved | Inflight::WaitingForReply(_))
        ));

        self.entries.insert(key, Inflight::WaitingForReply(waker));
    }

    pub fn complete_call(
        &mut self,
        handle_id: HandleId,
        mid: MessageId,
        info: MessageInfo,
        arg1: usize,
        arg2: usize,
    ) {
        let key = (handle_id, mid);
        debug_assert!(matches!(
            self.entries.get(&key),
            Some(Inflight::WaitingForReply(_))
        ));

        self.entries
            .insert(key, Inflight::Received { info, arg1, arg2 });
    }

    pub fn remove_if_completed(
        &mut self,
        handle_id: HandleId,
        mid: MessageId,
    ) -> Option<(MessageInfo, usize, usize)> {
        let key = (handle_id, mid);
        match self.entries.get(&key) {
            Some(Inflight::Received { info, arg1, arg2 }) => {
                let pair = (*info, *arg1, *arg2);
                self.entries.remove(&key);
                Some(pair)
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
enum Error {
    Sink(ErrorCode),
}

struct Executor {
    tasks: spin::Mutex<HashMap<TaskId, Task>>,
    run_queue: Arc<RunQueue>,
    inflights: spin::Mutex<InflightMap>,
    sink: Sink,
}

impl Executor {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::Sink)?;
        Ok(Self {
            tasks: spin::Mutex::new(HashMap::new()),
            run_queue: Arc::new(RunQueue::new()),
            inflights: spin::Mutex::new(InflightMap::new()),
            sink,
        })
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + Send + Sync + 'static) {
        let mut tasks = self.tasks.lock();
        let task_id = TaskId(tasks.len() as u32);
        let waker = TaskWaker::new(task_id, self.run_queue.clone());
        let task = Task {
            future: Box::pin(future),
            waker: Waker::from(Arc::new(waker)),
        };
        tasks.insert(task_id, task);
        self.run_queue.push(task_id);
    }

    fn run_runnable_tasks(&mut self) {
        while let Some(task_id) = self.run_queue.pop() {
            let mut task = {
                let mut tasks = self.tasks.lock();
                tasks.remove(&task_id).unwrap()
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

    pub fn run(&mut self) {
        loop {
            self.run_runnable_tasks();
            let (id, event) = self.sink.wait().unwrap();
            match event {
                Event::Message { info, arg1, arg2 } => {
                    self.inflights
                        .lock()
                        .complete_call(id, info.mid(), info, arg1, arg2);
                }
                Event::PeerClosed => {
                    todo!()
                }
            }
        }
    }
}

static GLOBAL_EXECUTOR: spin::Lazy<Executor> = spin::Lazy::new(|| Executor::new().unwrap());

pub struct Client(crate::channel::Channel);

impl Client {
    async fn open(&self, path: &[u8], options: OpenOptions) -> Result<Client, ErrorCode> {
        let (info, arg1, arg2) =
            CallFuture::call_with_body(&self.0, MessageKind::OPEN, options.as_usize(), path)?
                .await?;
        let handle = self.0.recv_handle(info)?;
        Ok(Client(Channel::from_handle(handle)))
    }
}

struct CallFuture {
    ch_id: HandleId,
    mid: MessageId,
}

impl CallFuture {
    fn call_with_body(
        ch: &Channel,
        kind: MessageKind,
        arg1: usize,
        body: &[u8],
    ) -> Result<Self, ErrorCode> {
        let ch_id = ch.handle().id();
        let mid = GLOBAL_EXECUTOR.inflights.lock().alloc_mid(ch_id)?;
        ch.send_body(kind, mid, body, arg1)?;
        Ok(Self { ch_id, mid })
    }
}

impl Future for CallFuture {
    type Output = Result<(MessageInfo, usize, usize), ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inflights = GLOBAL_EXECUTOR.inflights.lock();
        match inflights.remove_if_completed(self.ch_id, self.mid) {
            Some((info, arg1, arg2)) => Poll::Ready(Ok((info, arg1, arg2))),
            None => {
                inflights.set_waker(self.ch_id, self.mid, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub fn spawn(fut: impl Future<Output = ()> + Send + Sync + 'static) {
    GLOBAL_EXECUTOR.spawn(fut);
}

mod app {
    use super::Channel;
    use super::ChannelStream;
    use super::Completer;
    use super::Request;
    use super::Server;
    use super::spawn;

    async fn app_main() {
        let server = Server::serve("service/example").await.unwrap();
        loop {
            let conn = server.accept().await.unwrap();
            spawn(async move {
                loop {
                    let Some(request) = conn.next().await.unwrap() else {
                        break;
                    };

                    match request {
                        Request::Open { completer, path } => {
                            use alloc::vec;
                            let (ours, theirs) = crate::channel::Channel::new().unwrap();
                            let mut buf = vec![0; path.len()];
                            path.read(&mut buf).unwrap();
                            completer.reply(ours);
                            spawn(async move {
                                let mut stream = ChannelStream::new(theirs);
                                loop {
                                    let Some(request) = stream.next().await.unwrap() else {
                                        break;
                                    };
                                }
                            });
                        }
                        Request::Read {
                            completer,
                            offset,
                            len,
                        } => {
                            completer.reply(b"Hello, world!");
                        }
                        Request::Write {
                            completer,
                            offset,
                            data,
                        } => {
                            //
                        }
                    }
                }
            });
        }
    }
}

pub struct Server(crate::channel::Channel);

impl Server {
    pub async fn serve(service_name: &str) -> Result<Server, ErrorCode> {
        // TODO: supervisor register
        todo!()
    }

    pub async fn accept(&self) -> Result<ChannelStream, ErrorCode> {
        todo!()
    }
}

pub mod server {
    pub struct Server;
    pub enum Event {}
    pub struct EventStream;
}

pub enum ServerEvent {
    Accept { ch: Channel },
}

pub struct ChannelStream(crate::channel::Channel);

impl ChannelStream {
    pub fn new(ch: crate::channel::Channel) -> Self {
        Self(ch)
    }

    async fn next(&self) -> Result<Option<Request>, ErrorCode> {
        todo!()
    }
}

pub enum Request {
    Open {
        completer: Completer<Channel>,
        path: RequestBody,
    },
    Read {
        completer: Completer<[u8]>,
        offset: usize,
        len: usize,
    },
    Write {
        completer: Completer<usize>,
        offset: usize,
        data: RequestBody,
    },
}

pub struct Completer<T: ?Sized> {
    ch: Arc<Channel>,
    mid: MessageId,
    reply_kind: MessageKind,
    _pd: PhantomData<T>,
}

impl<T> Completer<T> {
    fn new(ch: Arc<Channel>, mid: MessageId, reply_kind: MessageKind) -> Self {
        Self {
            ch,
            mid,
            reply_kind,
            _pd: PhantomData,
        }
    }

    fn error(self, error: ErrorCode) {
        self.ch.reply_error(self.mid, error);
    }
}

impl Completer<usize> {
    pub fn reply(self, value: usize) -> Result<(), ErrorCode> {
        self.ch.send_args(self.reply_kind, self.mid, value, 0)
    }
}

impl Completer<Channel> {
    pub fn reply(self, channel: Channel) -> Result<(), ErrorCode> {
        self.ch
            .send_handle(self.reply_kind, self.mid, channel.into_handle())
    }
}

impl Completer<[u8]> {
    pub fn reply(self, data: &[u8]) -> Result<(), ErrorCode> {
        self.ch.send_body(self.reply_kind, self.mid, data, 0)
    }
}

pub struct RequestBody {
    ch: Arc<Channel>,
    len: usize,
    info: MessageInfo,
    read: bool,
}

impl RequestBody {
    fn new(ch: Arc<Channel>, len: usize, info: MessageInfo) -> Self {
        Self {
            ch,
            len,
            info,
            read: false,
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn read(mut self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        debug_assert!(!self.read);
        self.read = true;
        self.ch.recv_body(self.info, buf)
    }
}

impl Drop for RequestBody {
    fn drop(&mut self) {
        if !self.read {
            // TODO: Discard the message
        }
    }
}
