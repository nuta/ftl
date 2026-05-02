use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::MessageId;
use ftl_types::channel::OpenOptions;
use ftl_types::channel::Peek;
use ftl_types::channel::RecvToken;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::aio::executor::GLOBAL_EXECUTOR;
use crate::channel::Channel;
use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::message::Incoming;
use crate::message::Message;
use crate::message::OpenReply;
use crate::message::ReadReply;
use crate::message::WriteReply;

struct IdAllocator {
    bitmap: u64,
}

impl IdAllocator {
    const fn new() -> Self {
        Self { bitmap: 0 }
    }

    fn is_full(&self) -> bool {
        self.bitmap == u64::MAX
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

    pub fn handle_message(&self, handle_id: HandleId, peek: Peek) {
        let mut states = self.states.lock();
        let state = states
            .get_mut(&handle_id)
            .expect("missing async state for channel");

        let mid = peek.info.mid();
        // TODO: Check if the message is a reply type.
        if let Some(call) = state.inflight_calls.remove(&mid) {
            match call {
                InflightCall::WaitingForReply(waker) => {
                    waker.wake();
                    state
                        .inflight_calls
                        .insert(mid, InflightCall::Done { mid, peek });
                }
                InflightCall::Done { .. } => {
                    unreachable!("inflight call is already done");
                }
            }
        } else {
            state.peeks.push_back(peek);
            if let Some(waker) = state.pending_recvs.pop_front() {
                waker.wake();
            }
        }
    }

    fn use_channel<F, R>(&self, ch: &Channel, f: F) -> R
    where
        F: FnOnce(&mut ChannelState) -> R,
    {
        let mut states = self.states.lock();
        let state = states
            .get_mut(&ch.handle().id())
            .expect("missing async state for channel");
        f(state)
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
        GLOBAL_EXECUTOR.channels.use_channel(&self.ch, |state| {
            if let Some(peek) = state.peeks.pop_front() {
                let incoming = Incoming::parse(self.ch.clone(), peek);
                Poll::Ready(Ok(incoming))
            } else {
                state.pending_recvs.push_back(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

enum InflightCall {
    WaitingForReply(Waker),
    Done { mid: MessageId, peek: Peek },
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

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        GLOBAL_EXECUTOR.channels.use_channel(this.ch, |e| {
            match &mut this.state {
                CallState::NeedsMid(msg) => {
                    let Some(new_mid) = e.mid_allocator.alloc() else {
                        e.pending_sends.push_back(cx.waker().clone());
                        return Poll::Pending;
                    };

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
                        this.state = CallState::Done;
                        return Poll::Ready(Err(error));
                    }

                    e.inflight_calls
                        .insert(new_mid, InflightCall::WaitingForReply(cx.waker().clone()));

                    this.state = CallState::WaitingForReply(new_mid);
                    Poll::Pending
                }
                CallState::WaitingForReply(new_mid) => {
                    let call: &mut InflightCall = e
                        .inflight_calls
                        .get_mut(new_mid)
                        .expect("missing inflight call");

                    match call {
                        InflightCall::WaitingForReply(waker) => {
                            *waker = cx.waker().clone();
                            this.state = CallState::WaitingForReply(*new_mid);
                            Poll::Pending
                        }
                        InflightCall::Done { mid, peek } => {
                            let mid = *mid;

                            e.mid_allocator.free(mid);
                            if let Some(waker) = e.pending_sends.pop_front() {
                                waker.wake();
                            }

                            let Some(InflightCall::Done { mid: _, peek }) =
                                e.inflight_calls.remove(&mid)
                            else {
                                unreachable!();
                            };

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

struct Client {
    ch: Arc<Channel>,
}

impl Client {
    pub fn new(ch: impl Into<Arc<Channel>>) -> Self {
        Self { ch: ch.into() }
    }

    pub async fn open(&self, path: &[u8], options: OpenOptions) -> Result<OwnedHandle, ErrorCode> {
        let msg = Message::Open {
            mid: MessageId::new(0),
            path,
            options,
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        let reply = OpenReply::new(&self.ch, peek);
        let handle = reply.recv()?;
        Ok(handle)
    }

    pub async fn read<'a>(&self, offset: usize, buf: &'a mut [u8]) -> Result<&'a [u8], ErrorCode> {
        let msg = Message::Read {
            mid: MessageId::new(0),
            offset,
            len: buf.len(),
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        let reply = ReadReply::new(&self.ch, peek);
        let buf = reply.recv(buf)?;
        Ok(buf)
    }

    pub async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize, ErrorCode> {
        let msg = Message::Write {
            mid: MessageId::new(0),
            offset,
            buf,
        };

        let peek = CallFuture::new(self.ch.as_ref(), msg).await?;
        let reply = WriteReply::new(&self.ch, peek);
        Ok(reply.written_len())
    }
}
