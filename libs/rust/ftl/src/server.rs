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

pub trait Server {
    fn request(&mut self, ch: &Channel, request: Request, completer: Completer);
    fn response(&mut self, ch: &Channel, response: Response);
}
