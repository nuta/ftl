#![allow(unused)]
use ftl_types::error::ErrorCode;

pub struct Uri;
pub struct Buffer;

pub struct Channel;

pub enum Request {
    Open { uri: Uri },
    Read { len: usize },
    Write { buf: Buffer },
}

pub enum Response {
    Error { error: ErrorCode },
    Open { ch: Channel },
    Read { len: usize },
    Write { len: usize },
}

pub struct Completer;

pub struct Cookie;

pub trait Server {
    fn request(&mut self, ctx: &dyn Context, ch: &Channel, request: Request, completer: Completer);
    fn response(&mut self, ctx: &dyn Context, ch: &Channel, response: Response, cookie: Cookie);
}

pub trait Context<'a> {
    fn ch(&self) -> &'a Channel;
}
