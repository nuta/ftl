use ftl_driver::env::Env;

use crate::virtqueue::VirtQueue;

#[derive(Debug)]
pub enum Error {
    QueueSizeZero,
    TooHighPAddr,
    AllocFailed,
}

pub struct IsrStatus(pub(crate) u8);

impl IsrStatus {
    pub fn virtqueue_updated(&self) -> bool {
        self.0 & 1 != 0
    }
}

pub trait VirtioTransport: Send + Sync {
    fn acknowledge(&self, env: &dyn Env);

    fn read_device_features(&self, env: &dyn Env) -> u32;

    fn write_guest_features(&self, env: &dyn Env, guest_features: u32);

    fn driver_ok(&self, env: &dyn Env);

    fn setup_virtqueue<C>(&self, env: &dyn Env, queue_index: u16) -> Result<VirtQueue<C>, Error>;

    fn read_device_config8(&self, env: &dyn Env, offset: u16) -> u8;

    fn read_isr(&self, env: &dyn Env) -> IsrStatus;

    fn notify<C>(&self, env: &dyn Env, virtqueue: &VirtQueue<C>);
}
