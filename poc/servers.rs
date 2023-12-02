use std::{collections::HashMap, net::Ipv4Addr};

// use crate::ftl::*;

// struct Pcb {
//     local_port: Option<u16>,
// }

// enum Tracked {
//     Signal(Signal),
//     Houston(Channel),
//     Client { ch: Channel, pcb: Rc<RefCell<Pcb>> },
//     Data { ch: Channel, pcb: Rc<RefCell<Pcb>> },
//     Ip(Channel),
// }

// pub fn udp_main(env: Environ) {
//     let mut eventq = EventQueue::new();
//     let mut handles: HashMap<Handle, Tracked> = HashMap::new();

//     eventq.listen(env.signal, Ready::Readable);
//     handles.insert(env.signal, Tracked::Signal(env.signal));
//     eventq.listen(env.deps.houston.ch, Ready::Readable);
//     handles.insert(env.deps.houston.ch, Tracked::Houston(env.deps.houston_ch));
//     eventq.listen(env.deps.ip.ch, Ready::Readable);
//     handles.insert(env.deps.ip.ch, Tracked::Ip(env.deps.ip_ch));

//     // TODO:  export here

//     // Wait for a handle to be ready...
//     for Event { handle, ready } in eventq.into_infinite_iter() {
//         // Handle the ready handle.
//         match (&mut handles[&handle], ready) {
//             (Tracked::Houston { ch }, Ready::Readable) => match ch.recv() {
//                 Message::New => {
//                     let new_ch = Channel::new();
//                     let handle = new_ch.handle();

//                     eventq.listen(&handle, Ready::Readable);
//                     handles.insert(
//                         handle,
//                         Tracked::Client {
//                             ch: new_ch,
//                             pcb: Rc::new(RefCell::new(Pcb { local_port: None })),
//                         },
//                     );

//                     let reply = Message::NewReply { ch: handle };
//                     ch.send(reply);
//                 }
//                 _ => {
//                     error!("unexpected message: {:?}", m);
//                 }
//             },
//             // Read a message from the client, process it, and reply.
//             (Tracked::Client { ch, pcb }, Ready::Readable) => match ch.recv() {
//                 Message::Bind { local_port } => {
//                     let reply: Message = pcb.borrow_mut().bind(local_port).into();
//                     clients.send(reply);
//                 }
//                 _ => {
//                     error!("unexpected message: {:?}", m);
//                 }
//             },
//             _ => {
//                 error!("unexpected event: handle={:?}, ready={:?}", handle, ready);
//             }
//         }

//         // Remove the handle if it's closed.
//         if ready.closed {
//             handles.remove(&handle);
//             eventq.remove(&handle);
//         }
//     }
// }

struct Signal {}
impl Signal {
    fn handle(&self) -> Handle {
        todo!()
    }
}

struct Handle {}

struct Channel {}

impl Channel {
    fn new() -> Self {
        Self {}
    }

    fn handle(&self) -> Handle {
        todo!()
    }

    fn send(&mut self, msg: Message) {
        todo!()
    }
}

struct Houston {
    ch: Channel,
}

struct Ip {
    ch: Channel,
}

pub struct Deps {
    houston: Houston,
    ip: Ip,
}

enum Tracked {
    Signal(Signal),
    Houston { ch: Channel },
    Ip { ch: Channel },
    Client { ch: Channel },
}

pub struct Environ {
    signal: Signal,
    deps: Deps,
}

enum Ready {
    Readable,
}

enum Message {
    Ok,
    New,
    NewReply { ch: Channel },
    Bind { local_port: u16 },
}

enum Event {
    Message(Message),
}

struct MainLoop<T> {
    // ...
    tracks: HashMap<usize, T>,
}

impl<T> MainLoop<T> {
    fn new() -> Self {
        Self {
            tracks: HashMap::new(),
        }
    }

    fn listen(&mut self, ch: Handle, ready: Ready, tracked: T) {
        todo!()
    }

    fn next(&mut self) -> (&mut T, Event) {
        let ch = todo!();
        let msg = todo!();
        (&mut self.tracks[ch], Event::Message(msg))
    }
}

pub fn udp_main(env: Environ) {
    let mut mainloop = MainLoop::new();
    mainloop.listen(
        env.signal.handle(),
        Ready::Readable,
        Tracked::Signal(env.signal),
    );
    mainloop.listen(
        env.deps.houston.ch.handle(),
        Ready::Readable,
        Tracked::Houston {
            ch: env.deps.houston.ch,
        },
    );
    mainloop.listen(
        env.deps.ip.ch.handle(),
        Ready::Readable,
        Tracked::Ip { ch: env.deps.ip.ch },
    );

    loop {
        match mainloop.next() {
            (Tracked::Houston { ch, .. }, Event::Message(Message::New)) => {
                let new_ch = Channel::new();
                let handle = new_ch.handle();
                ch.send(Message::NewReply { ch: todo!() });
                mainloop.listen(
                    new_ch.handle(),
                    Ready::Readable,
                    Tracked::Client { ch: new_ch },
                );
            }
            (_, _) => {
                println!("unexpected event");
            }
        }
    }
}
