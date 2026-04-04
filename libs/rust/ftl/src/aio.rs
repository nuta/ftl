//! Async I/O support (`async fn`).

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::task::Wake;
use core::fmt;
use core::future;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::Attr;
use ftl_types::channel::MessageId;
use ftl_types::channel::MessageInfo;
use ftl_types::channel::MessageKind;
use ftl_types::channel::OpenOptions;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;
use log::warn;

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

enum CallState {
    Reserved,
    WaitingForReply(Waker),
    Received {
        info: MessageInfo,
        arg1: usize,
        arg2: usize,
    },
}

struct CallMap {
    entries: HashMap<(HandleId, MessageId), CallState>,
    next_mid: u16,
}

impl CallMap {
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
                self.entries.insert(key, CallState::Reserved);
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
            Some(CallState::Reserved | CallState::WaitingForReply(_))
        ));

        self.entries.insert(key, CallState::WaitingForReply(waker));
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
        let old = self
            .entries
            .insert(key, CallState::Received { info, arg1, arg2 });

        let Some(CallState::WaitingForReply(waker)) = old else {
            panic!(
                "unexpected call completion: handle_id={:?} mid={:?}",
                handle_id, mid
            );
        };

        waker.wake();
    }

    pub fn remove_if_completed(
        &mut self,
        handle_id: HandleId,
        mid: MessageId,
    ) -> Option<(MessageInfo, usize, usize)> {
        let key = (handle_id, mid);
        match self.entries.get(&key) {
            Some(CallState::Received { info, arg1, arg2 }) => {
                let pair = (*info, *arg1, *arg2);
                self.entries.remove(&key);
                Some(pair)
            }
            _ => None,
        }
    }
}

struct RecvEntry {
    info: MessageInfo,
    arg1: usize,
    arg2: usize,
}

enum RecvState {
    Waiting(Waker),
    Ready(VecDeque<RecvEntry>),
}

struct RecvMap {
    handles: HashMap<HandleId, RecvState>,
}

impl RecvMap {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    pub fn receive(&mut self, handle_id: HandleId, info: MessageInfo, arg1: usize, arg2: usize) {
        log::info!(
            "receive: handle_id={:?} info={:?} arg1={:?} arg2={:?}",
            handle_id,
            info,
            arg1,
            arg2
        );
        let entry = RecvEntry { info, arg1, arg2 };
        let state = self.handles.get_mut(&handle_id);
        match state {
            Some(RecvState::Waiting(waker)) => {
                waker.wake_by_ref();

                let mut queue = VecDeque::with_capacity(1);
                queue.push_back(entry);
                self.handles.insert(handle_id, RecvState::Ready(queue));
            }
            Some(RecvState::Ready(queue)) => {
                queue.push_back(entry);
            }
            None => {
                let mut queue = VecDeque::with_capacity(1);
                queue.push_back(entry);
                self.handles.insert(handle_id, RecvState::Ready(queue));
            }
        }
    }

    pub fn poll<'a, 'b>(&mut self, ch: &'a Channel, waker: &'b Waker) -> Option<Request<'a>> {
        let handle_id = ch.handle().id();
        log::info!("recv poll: handle_id={:?}", handle_id);
        let entry = match self.handles.get_mut(&handle_id) {
            Some(RecvState::Waiting(old_waker)) => {
                *old_waker = waker.clone();
                return None;
            }
            Some(RecvState::Ready(queue)) => {
                let entry = queue.pop_front().unwrap();
                if queue.is_empty() {
                    self.handles.remove(&handle_id);
                }
                entry
            }
            None => {
                self.handles
                    .insert(handle_id, RecvState::Waiting(waker.clone()));
                return None;
            }
        };

        let req = match entry.info.kind() {
            MessageKind::OPEN => {
                Request::Open {
                    path: Reader::new(ch, entry.info),
                    options: OpenOptions::from_usize(entry.arg1),
                    completer: Completer::new(ch, MessageKind::OPEN_REPLY, entry.info.mid()),
                }
            }
            MessageKind::READ => {
                Request::Read {
                    offset: entry.arg1,
                    len: entry.arg2,
                }
            }
            MessageKind::WRITE => {
                Request::Write {
                    offset: entry.arg1,
                    data: Reader::new(ch, entry.info),
                }
            }
            MessageKind::GETATTR => {
                Request::Getattr {
                    attr: Attr::from_usize(entry.arg1),
                }
            }
            MessageKind::SETATTR => {
                Request::Setattr {
                    attr: Attr::from_usize(entry.arg1),
                    data: Reader::new(ch, entry.info),
                }
            }
            _ => {
                warn!("unhandled message kind: {:?}", entry.info.kind());
                return None;
            }
        };

        Some(req)
    }
}

#[derive(Debug)]
enum Error {
    Sink(ErrorCode),
}

struct Executor {
    tasks: spin::Mutex<HashMap<TaskId, Task>>,
    run_queue: Arc<RunQueue>,
    calls: spin::Mutex<CallMap>,
    recvs: spin::Mutex<RecvMap>,
    sink: Sink,
}

impl Executor {
    pub fn new() -> Result<Self, Error> {
        let sink = Sink::new().map_err(Error::Sink)?;
        Ok(Self {
            tasks: spin::Mutex::new(HashMap::new()),
            run_queue: Arc::new(RunQueue::new()),
            calls: spin::Mutex::new(CallMap::new()),
            recvs: spin::Mutex::new(RecvMap::new()),
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

    fn run_runnable_tasks(&self) {
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

    pub fn run(&self) {
        loop {
            self.run_runnable_tasks();
            let (id, event) = self.sink.wait().unwrap();
            match event {
                Event::Message { info, arg1, arg2 } => {
                    if info.is_reply() {
                        self.calls
                            .lock()
                            .complete_call(id, info.mid(), info, arg1, arg2);
                    } else {
                        self.recvs.lock().receive(id, info, arg1, arg2);
                    }
                }
                Event::PeerClosed => {
                    todo!()
                }
            }
        }
    }
}

static GLOBAL_EXECUTOR: spin::Lazy<Executor> = spin::Lazy::new(|| Executor::new().unwrap());

pub fn spawn(future: impl Future<Output = ()> + Send + Sync + 'static) {
    GLOBAL_EXECUTOR.spawn(future);
}

pub fn run(future: impl Future<Output = ()> + Send + Sync + 'static) {
    spawn(future);
    GLOBAL_EXECUTOR.run();
}

pub struct Client(Channel);

impl Client {
    pub fn new(ch: Channel) -> Self {
        // FIXME:
        GLOBAL_EXECUTOR.sink.add(&ch).unwrap();
        Self(ch)
    }

    pub async fn open(&self, path: &[u8], options: OpenOptions) -> Result<Channel, ErrorCode> {
        let (info, arg1, arg2) =
            CallFuture::call_with_body(&self.0, MessageKind::OPEN, options.as_usize(), path)?
                .await?;
        let handle = self.0.recv_handle(info)?;
        Ok(Channel::from_handle(handle))
    }

    pub async fn write(&self, data: &[u8]) -> Result<usize, ErrorCode> {
        let offset = 0;
        let (info, written_len, _) =
            CallFuture::call_with_body(&self.0, MessageKind::WRITE, offset, data)?.await?;
        let _ = self.0.recv_args(info)?;
        Ok(written_len)
    }
}

pub struct Reader<'a> {
    ch: &'a Channel,
    msginfo: MessageInfo,
}

impl<'a> Reader<'a> {
    pub fn new(ch: &'a Channel, msginfo: MessageInfo) -> Self {
        Self { ch, msginfo }
    }

    pub fn len(&self) -> usize {
        self.msginfo.body_len()
    }

    pub fn read_all(self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        self.ch.recv_body(self.msginfo, buf)?;
        Ok(())
    }
}

pub struct Completer<'a, T> {
    ch: &'a Channel,
    kind: MessageKind,
    mid: MessageId,
    _pd: PhantomData<T>,
}

impl<'a, T> Completer<'a, T> {
    pub fn new(ch: &'a Channel, kind: MessageKind, mid: MessageId) -> Self {
        Self {
            ch,
            kind,
            mid,
            _pd: PhantomData,
        }
    }
}

impl<'a> Completer<'a, Channel> {
    pub fn reply(self, ch: Channel) -> Result<(), ErrorCode> {
        self.ch.send_handle(self.kind, self.mid, ch.into_handle())?;
        Ok(())
    }
}

pub enum Request<'a> {
    Open {
        path: Reader<'a>,
        options: OpenOptions,
        completer: Completer<'a, Channel>,
    },
    Read {
        offset: usize,
        len: usize,
    },
    Write {
        offset: usize,
        data: Reader<'a>,
    },
    Getattr {
        attr: Attr,
    },
    Setattr {
        attr: Attr,
        data: Reader<'a>,
    },
}

impl<'a> fmt::Debug for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::Open { options, .. } => {
                f.debug_struct("Open").field("options", options).finish()
            }
            Request::Read { offset, len } => {
                f.debug_struct("Read")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
            Request::Write { offset, data } => {
                f.debug_struct("Write").field("offset", offset).finish()
            }
            Request::Getattr { attr } => f.debug_struct("Getattr").field("attr", attr).finish(),
            Request::Setattr { attr, data } => {
                f.debug_struct("Setattr").field("attr", attr).finish()
            }
        }
    }
}

pub struct Server {
    ch: Channel,
}

impl Server {
    pub fn new(ch: Channel) -> Self {
        // FIXME:
        GLOBAL_EXECUTOR.sink.add(&ch).unwrap();
        Self { ch }
    }

    pub async fn recv(&self) -> Result<Request<'_>, ErrorCode> {
        RecvFuture::new(&self.ch).await
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
        arg: usize,
        body: &[u8],
    ) -> Result<Self, ErrorCode> {
        let ch_id = ch.handle().id();
        let mid = GLOBAL_EXECUTOR.calls.lock().alloc_mid(ch_id)?;
        ch.send_body(kind, mid, body, arg)?;
        Ok(Self { ch_id, mid })
    }
}

impl Future for CallFuture {
    type Output = Result<(MessageInfo, usize, usize), ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inflights = GLOBAL_EXECUTOR.calls.lock();
        match inflights.remove_if_completed(self.ch_id, self.mid) {
            Some((info, arg1, arg2)) => Poll::Ready(Ok((info, arg1, arg2))),
            None => {
                inflights.set_waker(self.ch_id, self.mid, cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct RecvFuture<'a> {
    ch: &'a Channel,
}

impl<'a> RecvFuture<'a> {
    fn new(ch: &'a Channel) -> Self {
        Self { ch }
    }
}

impl<'a> Future for RecvFuture<'a> {
    type Output = Result<Request<'a>, ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut recvs = GLOBAL_EXECUTOR.recvs.lock();
        match recvs.poll(self.ch, cx.waker()) {
            Some(request) => Poll::Ready(Ok(request)),
            None => Poll::Pending,
        }
    }
}
