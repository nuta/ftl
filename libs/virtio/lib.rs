#![no_std]

use ftl_api::collections::Vec;
use ftl_api::prelude::*;
use ftl_api::println;
use ftl_api::sync::Arc;

use self::transports::IsrStatus;
use self::transports::VirtioTransport;
use self::virtqueue::VirtQueue;

pub mod transports;
pub mod virtqueue;

const VIRTIO_STATUS_ACK: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FEAT_OK: u8 = 8;
// const VIRTIO_F_VERSION_1: u64 = 1 << 32;

pub type DeviceType = u32;
pub const VIRTIO_DEVICE_TYPE_NET: DeviceType = 1;

#[derive(Debug)]
pub enum VirtioAttachError {
    UnexpectedDeviceType(DeviceType),
    MissingFeatures,
    MissingPciCommonCfg,
    MissingPciDeviceCfg,
    MissingPciIsrCfg,
    MissingPciNotifyCfg,
    FeatureNegotiationFailure,
    NotSupportedBarType,
}

pub struct VirtioDevice {
    transport: Box<dyn VirtioTransport>,
    virtqueues: Vec<VirtQueue>,
}

impl VirtioDevice {
    pub fn new(transport: Box<dyn VirtioTransport>) -> VirtioDevice {
        VirtioDevice {
            transport,
            virtqueues: Vec::new(),
        }
    }

    pub fn probe(&mut self) -> Option<DeviceType> {
        self.transport.probe()
    }

    pub fn is_modern(&mut self) -> bool {
        self.transport.is_modern()
    }

    /// Returns the `i`-th virtqueue.
    pub fn virtq(&self, i: u16) -> &VirtQueue {
        self.virtqueues.get(i as usize).unwrap()
    }

    /// Returns the `i`-th virtqueue.
    pub fn virtq_mut(&mut self, i: u16) -> &mut VirtQueue {
        self.virtqueues.get_mut(i as usize).unwrap()
    }

    fn set_device_status_bit(&mut self, new_bits: u8) {
        let status = self.transport.read_device_status();
        self.transport.write_device_status(status | new_bits);
    }

    pub fn initialize(
        &mut self,
        features: u64,
        num_virtqueues: u16,
    ) -> Result<(), VirtioAttachError> {
        // "3.1.1 Driver Requirements: Device Initialization"
        self.transport.write_device_status(0); // Reset the device.
        self.set_device_status_bit(VIRTIO_STATUS_ACK);
        self.set_device_status_bit(VIRTIO_STATUS_DRIVER);
        let device_features = self.transport.read_device_features();
        if (device_features & features) != features {
            println!(
                "virtio: feature negotiation failure: driver={:x}, device={:x}, unspported={:x}",
                features,
                device_features,
                features & !device_features
            );
            return Err(VirtioAttachError::MissingFeatures);
        }

        self.transport.write_driver_features(features);
        self.set_device_status_bit(VIRTIO_STATUS_FEAT_OK);

        if (self.transport.read_device_status() & VIRTIO_STATUS_FEAT_OK) == 0 {
            return Err(VirtioAttachError::FeatureNegotiationFailure);
        }

        // Initialize virtqueues.
        let mut virtqueues = Vec::new();
        for index in 0..num_virtqueues {
            virtqueues.push(VirtQueue::new(index, &mut *self.transport));
        }
        self.virtqueues = virtqueues;

        self.set_device_status_bit(VIRTIO_STATUS_DRIVER_OK);

        Ok(())
    }
}
