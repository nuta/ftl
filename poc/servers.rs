use std::{collections::HashMap, net::Ipv4Addr};

use crate::ftl::*;

struct Pcb {
    local_port: Option<u16>,
}

enum OwnedHandle {
    Houston(Channel),
    Client { ch: Channel, pcb: Rc<RefCell<Pcb>> },
    Data { ch: Channel, pcb: Rc<RefCell<Pcb>> },
    Ip(Channel),
}

pub fn udp_main(env: Environ) {
    let mut eventq = EventQueue::new();
    let mut handles: HashMap<Handle, OwnedHandle> = HashMap::new();
    // Wait for a handle to be ready...
    for Event { handle, ready } in eventq.into_infinite_iter() {
        // Handle the ready handle.
        match (&mut handles[&handle], ready) {
            (OwnedHandle::Houston { ch }, Ready::READABLE) => match ch.recv() {
                Message::New => {
                    let new_ch = Channel::new();
                    let handle = new_ch.handle();
                    handles.insert(
                        handle,
                        OwnedHandle::Client {
                            ch: new_ch,
                            pcb: Rc::new(RefCell::new(Pcb { local_port: None })),
                        },
                    );

                    let reply = Message::NewReply { ch: handle };
                    ch.send(reply);
                }
                _ => {
                    error!("unexpected message: {:?}", m);
                }
            },
            // Read a message from the client, process it, and reply.
            (OwnedHandle::Client { ch, pcb }, Ready::READABLE) => match ch.recv() {
                Message::Bind { local_port } => {
                    let reply: Message = pcb.borrow_mut().bind(local_port).into();
                    clients.send(reply);
                }
                _ => {
                    error!("unexpected message: {:?}", m);
                }
            },
            _ => {
                error!("unexpected event: handle={:?}, ready={:?}", handle, ready);
            }
        }

        // Remove the handle if it's closed.
        if ready.closed {
            handles.remove(&handle);
            eventq.remove(&handle);
        }
    }
}
