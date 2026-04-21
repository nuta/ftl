use alloc::sync::Arc;

use hashbrown::HashMap;

use crate::address::IpAddr;

struct Endpoint {
    addr: IpAddr,
    port: u16,
}

enum TransportProtocol {
    Tcp,
    Udp,
}

struct FiveTuple {
    remote: Option<Endpoint>,
    local: Option<Endpoint>,
    protocol: TransportProtocol,
}

trait AnySocket {}

pub struct SocketMap {
    inner: HashMap<Endpoint, Arc<dyn AnySocket>>,
}
