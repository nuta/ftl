use crate::poll::Poll;

pub struct Mainloop<State> {
    poll: Poll,
}

impl<State> Mainloop<State> {
    pub fn new() -> Mainloop<State> {
        Mainloop {
            poll: Poll::new().unwrap(),
        }
    }
}
