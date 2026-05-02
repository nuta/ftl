use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;

use ftl_types::channel::MessageId;
use ftl_types::channel::Peek;
use ftl_types::channel::RecvToken;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::aio::executor::GLOBAL_EXECUTOR;
use crate::channel::Channel;
use crate::handle::Handleable;
use crate::message::Incoming;
use crate::message::Message;

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

    pub fn handle_peek(&self, handle_id: HandleId, peek: Peek) {
        let mut states = self.states.lock();
        let state = states
            .get_mut(&handle_id)
            .expect("missing async state for channel");
        state.peeks.push_back(peek);

        if let Some(waker) = state.pending_recvs.pop_front() {
            waker.wake();
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
    token: RecvToken,
}

impl RecvFuture {
    pub fn new(ch: Arc<Channel>, token: RecvToken) -> Self {
        Self { ch, token }
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
    WaitingForReply {
        mid: MessageId,
        waker: Waker,
    },
    Done {
        mid: MessageId,
        token: RecvToken,
    }
}

enum CallState<'a> {
    NeedsMid(Message<'a>),
    WaitForReply(MessageId),
    Done,
}

pub struct CallFuture<'a> {
    state: CallState<'a>,
    ch: &'a Channel,
}

impl<'a> CallFuture<'a> {
    pub fn new(ch: &'a Channel, msg: Message<'a>) -> Self {
        Self {
            state: CallState::NeedsMid(msg),
            ch,
        }
    }
}

impl<'a> Future for CallFuture<'a> {
    type Output = Result<Message<'a>, ErrorCode>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        GLOBAL_EXECUTOR
            .channels
            .use_channel(&self.ch, |state| {
                match &mut self.state {
                    CallState::NeedsMid(msg) => {
                        let Some(mid) = state.mid_allocator.alloc() else {
                            state.pending_sends.push_back(cx.waker().clone());
                            return Poll::Pending;
                        };

                        // TODO: Build a message with the mid
                        if let Err(error) = self.ch.send(msg) {
                            self.state = CallState::Done;
                            return Poll::Ready(Err(error));
                        }

                        state.inflight_calls.insert(mid, InflightCall::WaitingForReply {
                            mid,
                            waker: cx.waker().clone(),
                        });

                        self.state = CallState::WaitForReply(mid);
                        Poll::Pending
                    }
                    CallState::WaitForReply(mid) => {
                        let call = state.inflight_calls.get_mut(mid).expect("missing inflight call");
                        match call {
                            InflightCall::WaitingForReply { mid, waker } => {
                                *waker = cx.waker().clone();
                                Poll::Pending
                            }
                            InflightCall::Done { mid, token } => {
                                self.ch.recv_args(*token)?;
                                let reply = todo!();

                                state.mid_allocator.free(*mid);
                                state.inflight_calls.remove(mid);
                                self.state = CallState::Done;
                                Poll::Ready(Ok(reply))
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
