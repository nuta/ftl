//! Virtio device driver (legacy).
//!
//! # References
//!
//! Latest but very long:
//! <https://docs.oasis-open.org/virtio/virtio/v1.3/csd01/virtio-v1.3-csd01.html>
//!
//! Old but covers legacy + PCI concisely:
//! <https://ozlabs.org/~rusty/virtio-spec/virtio-0.9.5.pdf>

#![no_std]

pub mod virtio_pci;
pub mod virtqueue;

pub use virtio_pci::VirtioPci;
pub use virtqueue::ChainEntry;
pub use virtqueue::HeadId;
pub use virtqueue::UsedChain;
pub use virtqueue::VirtQueue;
