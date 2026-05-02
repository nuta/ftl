use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::MessageId;
use ftl_types::channel::OpenOptions;
use ftl_types::channel::Peek;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;

use crate::aio::executor::GLOBAL_EXECUTOR;
use crate::channel::Channel;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::message::Incoming;
use crate::message::Message;

struct IdAllocator {
    bitmap: u64,
}

impl IdAllocator {
    const fn new() -> Self {
        Self { bitmap: 0 }
    }

    fn alloc(&mut self) -> Option<MessageId> {
        let i = self.bitmap.trailing_ones();
        if i == 64 {
            return None;
        }
        self.bitmap |= 1 << i;
        Some(MessageId::new(i as u16))
    }

    fn free(&mut self, mid: MessageId) {
        let index = mid.as_u16() as usize;
        debug_assert!(self.bitmap & (1 << index) != 0);
        self.bitmap &= !(1 << index);
    }
}

struct ChannelState {
    mid_allocator: IdAllocator,
    peeks: VecDeque<Peek>,
    pending_recvs: VecDeque<Waker>,
    pending_sends: VecDeque<Waker>,
    inflight_calls: HashMap<MessageId, InflightCall>,
    peer_closed: bool,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            mid_allocator: IdAllocator::new(),
            peeks: VecDeque::new(),
            pending_recvs: VecDeque::new(),
            pending_sends: VecDeque::new(),
            inflight_calls: HashMap::new(),
            peer_closed: false,
        }
    }

    fn alloc_mid(&mut self) -> Option<MessageId> {
        self.mid_allocator.alloc()
    }

    fn free_mid(&mut self, mid: MessageId) {
        self.mid_allocator.free(mid);
        if let Some(waker) = self.pending_sends.pop_front() {
            waker.wake();
        }
    }
}

pub struct ChannelAio {
    states: spin::Mutex<HashMap<HandleId, ChannelState>>,
}

impl ChannelAio {
    pub fn new() -> Self {
        Self {
            states: spin::Mutex::new(HashMap::new()),
        }
    }

    pub fn add(&self, ch: &Channel) -> bool {
        let mut states = self.states.lock();
        let id = ch.handle().id();
        if states.contains_key(&id) {
            false
        } else {
            states.insert(id, ChannelState::new());
            true
        }
    }

    pub fn handle_message(&self, handle_id: HandleId, peek: Peek) {
        self.lock_state_by_id(handle_id, |state| {
            let mid = peek.info.mid();
            // TODO: Check if the message is a reply type.
            if let Some(call) = state.inflight_calls.remove(&mid) {
                match call {
                    InflightCall::Pending(waker) => {
                        state.inflight_calls.insert(mid, InflightCall::Ready(peek));

                        waker.wake();
                    }
                    InflightCall::Ready { .. } => {
                        unreachable!("inflight call is already done");
                    }
                }
            } else {
                state.peeks.push_back(peek);
                if let Some(waker) = state.pending_recvs.pop_front() {
                    waker.wake();
                }
            }
        });
    }

    pub fn handle_peer_closed(&self, handle_id: HandleId) {
        self.lock_state_by_id(handle_id, |state| {
            state.peer_closed = true;

            // Do not drain pending sends here; peer has closed their send side,
            // but still may receive messages from us.
            for waker in state.pending_recvs.drain(..) {
                waker.wake();
            }

            for (_, call) in state.inflight_calls.drain() {
                if let InflightCall::Pending(waker) = call {
                    waker.clone().wake();
                }
            }
        });
    }

    fn lock_state_by_id<F, R>(&self, handle_id: HandleId, f: F) -> R
    where
        F: FnOnce(&mut ChannelState) -> R,
    {
        let mut states = self.states.lock();
        let state = states
            .get_mut(&handle_id)
            .expect("missing async state for channel");
        f(state)
    }

    fn lock_state<F, R>(&self, ch: &Channel, f: F) -> R
    where
        F: FnOnce(&mut ChannelState) -> R,
    {
        self.lock_state_by_id(ch.handle().id(), f)
    }
}

pub struct RecvFuture {
    ch: Arc<Channel>,
}

impl RecvFuture {
    pub fn new(ch: Arc<Channel>) -> Self {
        Self { ch }
    }
}

impl Future for RecvFuture {
    type Output = Result<Incoming<Arc<Channel>>, ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        GLOBAL_EXECUTOR.channels.lock_state(&self.ch, |state| {
            if let Some(peek) = state.peeks.pop_front() {
                let incoming = Incoming::parse(self.ch.clone(), peek);
                Poll::Ready(Ok(incoming))
            } else if state.peer_closed {
                Poll::Ready(Err(ErrorCode::PeerClosed))
            } else {
                state.pending_recvs.push_back(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

enum InflightCall {
    Pending(Waker),
    Ready(Peek),
}

enum CallState<'a> {
    NeedsMid(Option<Message<'a>>),
    WaitingForReply(MessageId),
    Done,
}

pub struct CallFuture<'a> {
    state: CallState<'a>,
    ch: &'a Channel,
}

impl<'a> CallFuture<'a> {
    pub fn new(ch: &'a Channel, msg: Message<'a>) -> Self {
        Self {
            state: CallState::NeedsMid(Some(msg)),
            ch,
        }
    }
}

impl<'a> Future for CallFuture<'a> {
    type Output = Result<Peek, ErrorCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        GLOBAL_EXECUTOR.channels.lock_state(this.ch, |e| {
            match &mut this.state {
                CallState::NeedsMid(msg) => {
                    let Some(new_mid) = e.alloc_mid() else {
                        e.pending_sends.push_back(cx.waker().clone());
                        return Poll::Pending;
                    };

                    // TODO: Does rustc optimize this away?
                    let mut msg = msg.take().unwrap();
                    match &mut msg {
                        Message::Open { mid, .. } => *mid = new_mid,
                        Message::Read { mid, .. } => *mid = new_mid,
                        Message::Write { mid, .. } => *mid = new_mid,
                        Message::Getattr { mid, .. } => *mid = new_mid,
                        Message::Setattr { mid, .. } => *mid = new_mid,
                        _ => unreachable!("not a request message"),
                    };

                    if let Err(error) = this.ch.as_ref().send(msg) {
                        e.free_mid(new_mid);
                        this.state = CallState::Done;
                        return Poll::Ready(Err(error));
                    }

                    e.inflight_calls
                        .insert(new_mid, InflightCall::Pending(cx.waker().clone()));

                    this.state = CallState::WaitingForReply(new_mid);
                    Poll::Pending
                }
                CallState::WaitingForReply(mid) => {
                    let Entry::Occupied(mut entry) = e.inflight_calls.entry(*mid) else {
                        unreachable!();
                    };

                    match entry.get_mut() {
                        InflightCall::Pending(waker) if e.peer_closed => {
                            // Peer has closed their channel. This means we'll never
                            // receive a reply. Abort the call.
                            entry.remove();
                            e.free_mid(*mid);
                            this.state = CallState::Done;

                            Poll::Ready(Err(ErrorCode::PeerClosed))
                        }
                        InflightCall::Pending(waker) => {
                            *waker = cx.waker().clone();
                            this.state = CallState::WaitingForReply(*mid);
                            Poll::Pending
                        }
                        InflightCall::Ready { .. } => {
                            let InflightCall::Ready(peek) = entry.remove() else {
                                unreachable!();
                            };

                            e.free_mid(*mid);
                            this.state = CallState::Done;
                            Poll::Ready(Ok(peek))
                        }
                    }
                }
                CallState::Done => {
                    panic!("call future is already done");
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct Client {
    ch: Arc<Channel>,
}

impl Client {
    pub fn new(ch: impl Into<Arc<Channel>>) -> Self {
        let ch = ch.into();
        GLOBAL_EXECUTOR.add_channel(ch.as_ref());
        Self { ch }
    }

    pub async fn open(&self, path: &[u8], options: OpenOptions) -> Result<OwnedHandle, ErrorCode> {
        let msg = Message::Open {
            mid: MessageId::new(0),
            path,
            options,
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        match Incoming::parse(self.ch.clone(), peek) {
            Incoming::OpenReply(reply) => reply.recv(),
            Incoming::ErrorReply(reply) => Err(reply.error()),
            _ => Err(ErrorCode::InvalidMessage),
        }
    }

    pub async fn read<'a>(&self, offset: usize, buf: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        let msg = Message::Read {
            mid: MessageId::new(0),
            offset,
            len: buf.len(),
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        match Incoming::parse(self.ch.clone(), peek) {
            Incoming::ReadReply(reply) => reply.recv(buf),
            Incoming::ErrorReply(reply) => Err(reply.error()),
            _ => Err(ErrorCode::InvalidMessage),
        }
    }

    pub async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize, ErrorCode> {
        let msg = Message::Write {
            mid: MessageId::new(0),
            offset,
            buf,
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        match Incoming::parse(self.ch.clone(), peek) {
            Incoming::WriteReply(reply) => Ok(reply.written_len()),
            Incoming::ErrorReply(reply) => Err(reply.error()),
            _ => Err(ErrorCode::InvalidMessage),
        }
    }
}
