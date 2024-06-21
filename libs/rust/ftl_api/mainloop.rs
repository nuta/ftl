use ftl_types::handle::HandleId;
use hashbrown::HashMap;

use crate::poll::Poll;

pub struct Mainloop<State> {
    poll: Poll,
    states: HashMap<HandleId, State>,
}

impl<State> Mainloop<State> {
    pub fn new() -> Mainloop<State> {
        Mainloop {
            poll: Poll::new().unwrap(),
            states: HashMap::new(),
        }
    }
}
