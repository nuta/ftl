#![no_std]
use core::mem::size_of;

use ftl_driver::dma::DmaBuf;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Error;
use ftl_driver::net::Event;
use ftl_driver::net::Notifier;
use ftl_driver::pci;
use ftl_driver::trace;
use ftl_driver::warn;
use ftl_utils::spinlock::SpinLock;
use ftl_virtio::ChainEntry;
use ftl_virtio::VirtQueue;
use ftl_virtio::VirtioPci;
use ftl_virtio::virtio_pci::DeviceType;

const VIRTIO_NET_F_MAC: u32 = 1 << 5;

#[derive(Debug)]
pub enum InitError {
    DeviceNotFound,
    Bar0NotIoSpace,
    MacNotAvailable,
    TxVqSetup(ftl_virtio::virtio_pci::Error),
    RxVqSetup(ftl_virtio::virtio_pci::Error),
}

#[repr(C, packed)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

struct TxData {
    header_buf: DmaBuf,
    payload_buf: DmaBuf,
}

struct RxData {
    buf: DmaBuf,
}

struct Mutable<N: Notifier> {
    txq: VirtQueue<TxData>,
    rxq: VirtQueue<RxData>,
    tx_notifier: Option<N>,
    rx_notifier: Option<N>,
}

pub struct VirtioNet<N: Notifier> {
    mac: [u8; 6],
    virtio: VirtioPci,
    mutable: SpinLock<Mutable<N>>,
}

impl<N: Notifier> VirtioNet<N> {
    pub fn init(env: &dyn Env) -> Result<Self, InitError> {
        let Some(dev) = pci::find_virtio_device(env, DeviceType::Network as u16) else {
            warn!(env, "virtio-net: device not found");
            return Err(InitError::DeviceNotFound);
        };

        trace!(
            env,
            "virtio-net: found at {:02x}:{:02x} (device_id={:#x}, subsystem={})",
            dev.bus,
            dev.slot,
            dev.device,
            dev.subsystem_id
        );

        pci::set_bus_master(env, &dev, true);

        let bar0 = pci::get_bar(env, &dev, 0);
        if bar0 & 1 == 0 {
            warn!(env, "virtio-net: BAR0 is not I/O space (modern-only?)");
            return Err(InitError::Bar0NotIoSpace);
        }
        let iobase = (bar0 & 0xffff_fffc) as u16;
        trace!(env, "virtio-net: iobase={iobase:#x}");

        let virtio = VirtioPci::new(iobase);
        virtio.acknowledge(env);

        let device_features = virtio.read_device_features(env);
        if device_features & VIRTIO_NET_F_MAC == 0 {
            warn!(env, "virtio-net: MAC feature not advertised");
            return Err(InitError::MacNotAvailable);
        }
        let guest_features = device_features & VIRTIO_NET_F_MAC;
        virtio.write_guest_features(env, guest_features);

        let mac = [
            virtio.read_device_config8(env, 0),
            virtio.read_device_config8(env, 1),
            virtio.read_device_config8(env, 2),
            virtio.read_device_config8(env, 3),
            virtio.read_device_config8(env, 4),
            virtio.read_device_config8(env, 5),
        ];
        trace!(env, "virtio-net: mac={mac:02x?}");

        let txq = virtio
            .setup_virtqueue(env, 1)
            .map_err(InitError::TxVqSetup)?;

        let rxq = virtio
            .setup_virtqueue(env, 0)
            .map_err(InitError::RxVqSetup)?;

        virtio.driver_ok(env);

        Ok(Self {
            mac,
            virtio,
            mutable: SpinLock::new(Mutable {
                txq,
                rxq,
                tx_notifier: None,
                rx_notifier: None,
            }),
        })
    }
}

impl<N: Notifier> Driver for VirtioNet<N> {
    type Notifier = N;

    fn mac_address(&self) -> &[u8; 6] {
        &self.mac
    }

    fn try_send(
        &self,
        env: &dyn Env,
        mut header_buf: DmaBuf,
        headroom: usize,
        payload_buf: DmaBuf,
    ) -> Result<(), (DmaBuf, DmaBuf, Error)> {
        // We need some space to prepend the Virtio-net header.
        if headroom < size_of::<VirtioNetHdr>() {
            return Err((header_buf, payload_buf, Error::HeadroomTooSmall));
        }

        if headroom >= header_buf.len() {
            return Err((header_buf, payload_buf, Error::HeadroomTooLarge));
        }

        let mut mutable = self.mutable.lock();

        // Fill the virtio-net header with zeros.
        let header_offset = headroom - size_of::<VirtioNetHdr>();
        header_buf.as_mut_slice()[header_offset..headroom].fill(0);

        // Prepare a descriptor chain for virtio-net.
        let header_entry = ChainEntry::Read {
            paddr: (header_buf.paddr() + header_offset) as u64,
            len: (header_buf.len() - header_offset) as u32,
        };
        let payload_entry = ChainEntry::Read {
            paddr: payload_buf.paddr() as u64,
            len: payload_buf.len() as u32,
        };
        let chain = [header_entry, payload_entry];
        let chain_len = if payload_buf.len() == 0 { 1 } else { 2 };
        let chain = &chain[..chain_len];

        if let Err((_, data)) = mutable.txq.push(
            chain,
            TxData {
                header_buf,
                payload_buf,
            },
        ) {
            return Err((data.header_buf, data.payload_buf, Error::TxFull));
        }

        self.virtio.notify(env, &mutable.txq);
        Ok(())
    }

    fn provide(&self, env: &dyn Env, buf: DmaBuf) -> Result<(), (Error, DmaBuf)> {
        let mut mutable = self.mutable.lock();

        let chain = [ChainEntry::Write {
            paddr: buf.paddr() as u64,
            len: buf.len() as u32,
        }];

        if let Err((_, RxData { buf })) = mutable.rxq.push(&chain, RxData { buf }) {
            return Err((Error::RxFull, buf));
        }

        self.virtio.notify(env, &mutable.rxq);
        Ok(())
    }

    fn try_receive(&self) -> Result<(DmaBuf, usize, usize), (Error, Option<DmaBuf>)> {
        let mut mutable = self.mutable.lock();
        let (buf, total_len) = match mutable.rxq.pop() {
            Ok(Some((RxData { buf }, total_len))) => (buf, total_len),
            Ok(None) => return Err((Error::RxEmpty, None)),
            Err(_) => return Err((Error::BadDevice, None)),
        };

        if total_len > buf.len() {
            return Err((Error::BadDevice, Some(buf)));
        }

        let header_size = size_of::<VirtioNetHdr>();
        let Some(payload_len) = total_len.checked_sub(header_size) else {
            return Err((Error::BadDevice, Some(buf)));
        };

        Ok((buf, header_size, payload_len))
    }

    fn subscribe_tx(&self, notifier: Self::Notifier) -> Result<(), Error> {
        let mut mutable = self.mutable.lock();

        if mutable.txq.can_push() {
            // The queue is already ready to push a packet.
            notifier.notify(Event::TxAvailable);
            return Ok(());
        }

        mutable.tx_notifier = Some(notifier);
        Ok(())
    }

    fn subscribe_rx(&self, notifier: Self::Notifier) -> Result<(), Error> {
        let mut mutable = self.mutable.lock();

        if mutable.rxq.can_pop() {
            // The queue is already ready to pop a packet.
            notifier.notify(Event::RxAvailable);
            return Ok(());
        }

        mutable.rx_notifier = Some(notifier);
        Ok(())
    }

    fn handle_interrupt(&self, env: &dyn Env) {
        let mut mutable = self.mutable.lock();
        let status = self.virtio.read_isr(env);
        if status.virtqueue_updated() {
            loop {
                match mutable.txq.pop() {
                    Ok(Some((data, _total_len))) => {
                        env.free_dma(data.header_buf);
                        env.free_dma(data.payload_buf);
                    }
                    Ok(None) => break,
                    // Ignore bad descriptors.
                    Err(_) => continue,
                }
            }

            if mutable.txq.can_push() {
                if let Some(notifier) = mutable.tx_notifier.take() {
                    notifier.notify(Event::TxAvailable);
                }
            }

            if mutable.rxq.can_pop() {
                if let Some(notifier) = mutable.rx_notifier.take() {
                    notifier.notify(Event::RxAvailable);
                }
            }
        }
    }
}
