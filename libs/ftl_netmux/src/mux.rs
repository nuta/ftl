use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_types::error::ErrorCode;
use ftl_types::net::Rule;
use ftl_utils::fxhash::FxHashMap;

use crate::device::Device;
use crate::device::DeviceId;
use crate::device::PollNotifier;
use crate::device::Tx;
use crate::nic::BufReader;
use crate::nic::BufWriter;
use crate::nic::NicId;
use crate::rx::RxRouteTable;
use crate::tx::TxRouteTable;

pub trait RxNotify: Send + 'static {
    fn notify(self) -> Result<(), ErrorCode>;
}

pub struct NetMux<'a, N: RxNotify> {
    pub(crate) env: &'a dyn Env,
    next_device_id: u64,
    pub(crate) rx_buffer_size: usize,
    pub(crate) devices: FxHashMap<DeviceId, Device<'a>>,
    pub(crate) rx: RxRouteTable<'a, N>,
    pub(crate) tx: TxRouteTable,
}

impl<'a, N: RxNotify> NetMux<'a, N> {
    pub const fn new(env: &'a dyn Env, rx_buffer_size: usize) -> Self {
        Self {
            env,
            next_device_id: 0,
            rx_buffer_size,
            devices: FxHashMap::new(),
            rx: RxRouteTable::new(),
            tx: TxRouteTable::new(),
        }
    }

    pub fn add_device(
        &mut self,
        driver: &'a dyn Driver<Notifier = PollNotifier>,
    ) -> Result<DeviceId, ErrorCode> {
        self.devices
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;

        self.provide_rx_buffers(self.env, driver, 64);

        let id = self.alloc_device_id()?;
        self.devices.insert(id, Device::new(self.env, driver));
        Ok(id)
    }

    pub fn create_nic(&mut self) -> Result<NicId, ErrorCode> {
        self.rx.add_nic()
    }

    pub fn remove_nic(&mut self, nic: NicId) {
        let _nic = self.rx.remove_nic(nic);
    }

    pub fn bind(&mut self, nic: NicId, rule: Rule) -> Result<(), ErrorCode> {
        self.rx.bind(nic, rule)
    }

    pub fn unbind(&mut self, nic: NicId, rule: &Rule) -> Result<(), ErrorCode> {
        self.rx.unbind(nic, rule)
    }

    pub fn subscribe(&mut self, nic: NicId, notifier: N) -> Result<(), ErrorCode> {
        self.rx
            .get_nic(nic)
            .ok_or(ErrorCode::NotFound)?
            .subscribe(notifier)
    }

    pub fn start_dhcp(&mut self, device: DeviceId) {
        let Some(device) = self.devices.get_mut(&device) else {
            return;
        };

        device.start_dhcp();
    }

    /// Sends a packet to the network.
    pub fn send(
        &mut self,
        nic: NicId,
        header: &mut dyn BufReader,
        payload: Option<&mut dyn BufReader>,
    ) -> Result<(), ErrorCode> {
        let payload_len = if let Some(payload) = payload.as_ref() {
            payload.len()
        } else {
            0
        };

        let mut tx = Tx::alloc(self.env, header.len(), payload_len)?;
        let (device_id, our_ip, next_hop_ip) = self.prepare_tx(nic, &mut tx, header, payload)?;

        // Send the packet through the route's next hop.
        let Some(device) = self.devices.get(&device_id) else {
            return Err(ErrorCode::NotFound);
        };

        device.send_ipv4(our_ip, next_hop_ip, tx)
    }

    pub fn recv(
        &mut self,
        nic: NicId,
        header: &mut dyn BufWriter,
        payload: &mut dyn BufWriter,
    ) -> Result<usize, ErrorCode> {
        let nic = self.rx.get_nic(nic).ok_or(ErrorCode::NotFound)?;
        nic.recv(header, payload)
    }

    fn alloc_device_id(&mut self) -> Result<DeviceId, ErrorCode> {
        for _ in 0..64 {
            self.next_device_id = self.next_device_id.wrapping_add(1);
            let id = DeviceId::new(self.next_device_id);
            if !self.devices.contains_key(&id) {
                return Ok(id);
            }
        }

        Err(ErrorCode::OutOfBounds)
    }
}
