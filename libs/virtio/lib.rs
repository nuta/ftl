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
