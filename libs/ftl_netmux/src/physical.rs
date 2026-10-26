use alloc::boxed::Box;

use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_types::error::ErrorCode;
use ftl_types::time::MonoTime;

use crate::device::Device;
use crate::device::PollNotifier;
use crate::packet::ipv4::Ipv4Addr;
use crate::packet::ipv4::NetMask;

pub trait RxNotify: Send + 'static {
    fn notify(self) -> Result<(), ErrorCode>;
}

#[derive(Clone, Copy)]
pub(crate) struct ExternalIpConfig {
    pub(crate) address: Ipv4Addr,
    pub(crate) netmask: NetMask,
    pub(crate) gateway: Ipv4Addr,
}

impl ExternalIpConfig {
    pub(crate) fn next_hop_ip(&self, dst: Ipv4Addr) -> Ipv4Addr {
        if self.netmask.contains(self.address, dst) {
            dst
        } else {
            self.gateway
        }
    }
}

pub struct PhysicalState<'a> {
    pub(crate) env: &'a dyn Env,
    pub(crate) rx_buffer_size: usize,
    pub(crate) device: Device<'a>,
    pub(crate) external: Option<ExternalIpConfig>,
}

impl<'a> PhysicalState<'a> {
    pub fn new(
        env: &'a dyn Env,
        rx_buffer_size: usize,
        driver: Box<dyn Driver<Notifier = PollNotifier>>,
        irq: u8,
    ) -> Self {
        let this = Self {
            env,
            rx_buffer_size,
            device: Device::new(env, driver, irq),
            external: None,
        };

        this.provide_rx_buffers(env, this.device.driver(), 64);
        this
    }

    pub fn start_dhcp(&mut self, now: MonoTime) {
        self.device.start_dhcp(now);
    }
}
