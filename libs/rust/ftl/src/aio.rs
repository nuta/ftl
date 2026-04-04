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
use core::mem;
use core::pin::Pin;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;
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
    PeerClosed,
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
    ) -> Option<Result<(MessageInfo, usize, usize), CallError>> {
        let key = (handle_id, mid);
        match self.entries.get(&key) {
            Some(CallState::Received { info, arg1, arg2 }) => {
                let pair = (*info, *arg1, *arg2);
                self.entries.remove(&key);
                Some(Ok(pair))
            }
            Some(CallState::PeerClosed) => {
                self.entries.remove(&key);
                Some(Err(CallError::PeerClosed))
            }
            _ => None,
        }
    }

    pub fn peer_closed(&mut self, handle_id: HandleId) {
        for ((id, _mid), value) in self.entries.iter_mut() {
            if *id == handle_id {
                let old = mem::replace(value, CallState::PeerClosed);
                match old {
                    CallState::WaitingForReply(waker) => {
                        waker.wake();
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn close(&mut self, handle_id: HandleId) {
        self.entries.retain(|key, _| key.0 != handle_id);
    }
}

struct RecvEntry {
    info: MessageInfo,
    arg1: usize,
    arg2: usize,
}

enum RecvState {
    BeforeRecv,
    Waiting(Waker),
    Ready(VecDeque<RecvEntry>),
    Draining(VecDeque<RecvEntry>),
    PeerClosed,
}

struct RecvMap {
    states: HashMap<HandleId, RecvState>,
}

impl RecvMap {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn add(&mut self, handle_id: HandleId) {
        self.states.insert(handle_id, RecvState::BeforeRecv);
    }

    pub fn receive(&mut self, handle_id: HandleId, info: MessageInfo, arg1: usize, arg2: usize) {
        let entry = RecvEntry { info, arg1, arg2 };
        let state = self.states.get_mut(&handle_id);
        match state {
            Some(RecvState::BeforeRecv) => {
                let mut queue = VecDeque::with_capacity(1);
                queue.push_back(entry);
                self.states.insert(handle_id, RecvState::Ready(queue));
            }
            Some(RecvState::Waiting(waker)) => {
                waker.wake_by_ref();

                let mut queue = VecDeque::with_capacity(1);
                queue.push_back(entry);
                self.states.insert(handle_id, RecvState::Ready(queue));
            }
            Some(RecvState::Ready(queue)) => {
                queue.push_back(entry);
            }
            Some(RecvState::Draining(_) | RecvState::PeerClosed) => {
                unreachable!();
            }
            None => {
                let mut queue = VecDeque::with_capacity(1);
                queue.push_back(entry);
                self.states.insert(handle_id, RecvState::Ready(queue));
            }
        }
    }

    pub fn poll<'a, 'b>(
        &mut self,
        ch: &'a Channel,
        waker: &'b Waker,
    ) -> Option<Result<Request<'a>, ErrorCode>> {
        let handle_id = ch.handle().id();
        let entry = match self.states.get_mut(&handle_id).unwrap() {
            RecvState::BeforeRecv => {
                self.states
                    .insert(handle_id, RecvState::Waiting(waker.clone()));
                return None;
            }
            RecvState::Waiting(old_waker) => {
                // TODO: Should we support multiple receivers?
                *old_waker = waker.clone();
                return None;
            }
            RecvState::Ready(queue) => {
                let entry = queue.pop_front().unwrap();
                if queue.is_empty() {
                    self.states
                        .insert(handle_id, RecvState::Waiting(waker.clone()));
                }
                entry
            }
            RecvState::Draining(queue) => {
                let entry = queue.pop_front().unwrap();
                if queue.is_empty() {
                    self.states.insert(handle_id, RecvState::PeerClosed);
                }
                entry
            }
            RecvState::PeerClosed => {
                return Some(Err(ErrorCode::PeerClosed));
            }
        };

        let req = match entry.info.kind() {
            MessageKind::OPEN => {
                let reader = Reader::new(ch, entry.info);
                Request::Open {
                    path: reader,
                    options: OpenOptions::from_usize(entry.arg1),
                    completer: Completer::new(ch, MessageKind::OPEN_REPLY, entry.info.mid()),
                }
            }
            MessageKind::READ => {
                if let Err(err) = ch.recv_args(entry.info) {
                    // TODO: Should we return an error to the caller?
                    warn!("failed to recv read message: {:?}", err);
                }

                Request::Read {
                    offset: entry.arg1,
                    len: entry.arg2,
                    completer: Completer::new(ch, MessageKind::READ_REPLY, entry.info.mid()),
                }
            }
            MessageKind::WRITE => {
                let reader = Reader::new(ch, entry.info);
                Request::Write {
                    offset: entry.arg1,
                    data: reader,
                    completer: Completer::new(ch, MessageKind::WRITE_REPLY, entry.info.mid()),
                }
            }
            MessageKind::GETATTR => {
                if let Err(err) = ch.recv_args(entry.info) {
                    // TODO: Should we return an error to the caller?
                    warn!("failed to recv getattr message: {:?}", err);
                }

                Request::GetAttr {
                    attr: Attr::from_usize(entry.arg1),
                    completer: Completer::new(ch, MessageKind::GETATTR_REPLY, entry.info.mid()),
                }
            }
            MessageKind::SETATTR => {
                let reader = Reader::new(ch, entry.info);
                Request::SetAttr {
                    attr: Attr::from_usize(entry.arg1),
                    data: reader,
                    completer: Completer::new(ch, MessageKind::SETATTR_REPLY, entry.info.mid()),
                }
            }
            _ => {
                warn!("unhandled message kind: {:?}", entry.info.kind());
                return None;
            }
        };

        Some(Ok(req))
    }

    pub fn peer_closed(&mut self, handle_id: HandleId) {
        match self.states.remove(&handle_id) {
            Some(RecvState::BeforeRecv) => {
                self.states.insert(handle_id, RecvState::PeerClosed);
            }
            Some(RecvState::Waiting(waker)) => {
                waker.wake_by_ref();
                self.states.insert(handle_id, RecvState::PeerClosed);
            }
            Some(RecvState::Ready(queue)) => {
                self.states.insert(handle_id, RecvState::Draining(queue));
            }
            Some(RecvState::Draining(_)) | Some(RecvState::PeerClosed) => {
                unreachable!();
            }
            None => {}
        }
    }

    pub fn close(&mut self, handle_id: HandleId) {
        self.states.remove(&handle_id);
    }
}

#[derive(Debug)]
enum Error {
    Sink(ErrorCode),
}

struct Executor {
    next_task_id: AtomicU32,
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
            next_task_id: AtomicU32::new(0),
            tasks: spin::Mutex::new(HashMap::new()),
            run_queue: Arc::new(RunQueue::new()),
            calls: spin::Mutex::new(CallMap::new()),
            recvs: spin::Mutex::new(RecvMap::new()),
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
            // TODO: When should we exit the loop?
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
                    self.calls.lock().peer_closed(id);
                    self.recvs.lock().peer_closed(id);
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

#[derive(Debug)]
pub enum CallError {
    Syscall(ErrorCode),
    PeerClosed,
    ErrorReply(ErrorCode),
}

pub struct Client(Channel);

impl Client {
    pub fn new(ch: Channel) -> Self {
        // FIXME:
        GLOBAL_EXECUTOR.sink.add(&ch).unwrap();
        Self(ch)
    }

    pub async fn open(&self, path: &[u8], options: OpenOptions) -> Result<Channel, CallError> {
        let (info, arg1, arg2) =
            CallFuture::call_with_body(&self.0, MessageKind::OPEN, options.as_usize(), path)?
                .await?;
        let handle = self.0.recv_handle(info).map_err(CallError::Syscall)?;
        Ok(Channel::from_handle(handle))
    }

    pub async fn write(&self, data: &[u8]) -> Result<usize, CallError> {
        let offset = 0;
        let (info, written_len, _) =
            CallFuture::call_with_body(&self.0, MessageKind::WRITE, offset, data)?.await?;
        let _ = self.0.recv_args(info).map_err(CallError::Syscall)?;
        Ok(written_len)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        GLOBAL_EXECUTOR.calls.lock().close(self.0.handle().id());
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

pub struct Completer<'a, T: ?Sized> {
    ch: &'a Channel,
    kind: MessageKind,
    mid: MessageId,
    _pd: PhantomData<T>,
}

impl<'a, T: ?Sized> Completer<'a, T> {
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

impl<'a> Completer<'a, [u8]> {
    pub fn reply(self, data: &[u8]) -> Result<(), ErrorCode> {
        self.ch.send_body(self.kind, self.mid, data, 0)?;
        Ok(())
    }
}

impl<'a> Completer<'a, usize> {
    pub fn reply(self, len: usize) -> Result<(), ErrorCode> {
        self.ch.send_args(self.kind, self.mid, len, 0)?;
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
        completer: Completer<'a, [u8]>,
    },
    Write {
        offset: usize,
        data: Reader<'a>,
        completer: Completer<'a, usize>,
    },
    GetAttr {
        attr: Attr,
        completer: Completer<'a, [u8]>,
    },
    SetAttr {
        attr: Attr,
        data: Reader<'a>,
        completer: Completer<'a, usize>,
    },
}

impl<'a> fmt::Debug for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::Open { options, .. } => {
                f.debug_struct("Open").field("options", options).finish()
            }
            Request::Read { offset, len, .. } => {
                f.debug_struct("Read")
                    .field("offset", offset)
                    .field("len", len)
                    .finish()
            }
            Request::Write { offset, data, .. } => {
                f.debug_struct("Write").field("offset", offset).finish()
            }
            Request::GetAttr { attr, .. } => f.debug_struct("Getattr").field("attr", attr).finish(),
            Request::SetAttr { attr, data, .. } => {
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

impl Drop for Server {
    fn drop(&mut self) {
        GLOBAL_EXECUTOR.recvs.lock().close(self.ch.handle().id());
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
    ) -> Result<Self, CallError> {
        let ch_id = ch.handle().id();
        let mid = GLOBAL_EXECUTOR
            .calls
            .lock()
            .alloc_mid(ch_id)
            .map_err(CallError::Syscall)?;
        ch.send_body(kind, mid, body, arg)
            .map_err(CallError::Syscall)?;
        Ok(Self { ch_id, mid })
    }
}

impl Future for CallFuture {
    type Output = Result<(MessageInfo, usize, usize), CallError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inflights = GLOBAL_EXECUTOR.calls.lock();
        match inflights.remove_if_completed(self.ch_id, self.mid) {
            Some(Ok((info, arg1, arg2))) => {
                let result = if info.kind() == MessageKind::ERROR_REPLY {
                    Err(CallError::ErrorReply(ErrorCode::from(arg1)))
                } else {
                    Ok((info, arg1, arg2))
                };

                Poll::Ready(result)
            }
            Some(Err(error)) => Poll::Ready(Err(error)),
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
        GLOBAL_EXECUTOR.recvs.lock().add(ch.handle().id());
        Self { ch }
    }
}

impl<'a> Future for RecvFuture<'a> {
    type Output = Result<Request<'a>, ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut recvs = GLOBAL_EXECUTOR.recvs.lock();
        match recvs.poll(self.ch, cx.waker()) {
            Some(Ok(request)) => Poll::Ready(Ok(request)),
            Some(Err(error)) => Poll::Ready(Err(error)),
            None => Poll::Pending,
        }
    }
}
