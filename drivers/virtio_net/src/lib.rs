#![no_std]
use core::mem::size_of;

use ftl_driver::dma::DmaBuf;
use ftl_driver::dma::DmaBufWithDrop;
use ftl_driver::env::Env;
use ftl_driver::net::Driver;
use ftl_driver::net::Error;
use ftl_driver::net::Event;
use ftl_driver::net::Notifier;
use ftl_driver::trace;
use ftl_driver::warn;
use ftl_utils::spinlock::SpinLock;
use ftl_virtio::ChainEntry;
use ftl_virtio::VirtQueue;
use ftl_virtio::VirtioTransport;

const VIRTIO_NET_F_MAC: u32 = 1 << 5;

#[derive(Debug)]
pub enum InitError {
    MacNotAvailable,
    TxVqSetup(ftl_virtio::Error),
    RxVqSetup(ftl_virtio::Error),
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
    payload_buf: Option<DmaBuf>,
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

pub struct VirtioNet<T: VirtioTransport, N: Notifier> {
    mac: [u8; 6],
    transport: T,
    mutable: SpinLock<Mutable<N>>,
}

impl<T: VirtioTransport, N: Notifier> VirtioNet<T, N> {
    pub fn init(env: &dyn Env, transport: T) -> Result<Self, InitError> {
        transport.acknowledge(env);

        let device_features = transport.read_device_features(env);
        if device_features & VIRTIO_NET_F_MAC == 0 {
            warn!(env, "MAC feature not advertised");
            return Err(InitError::MacNotAvailable);
        }
        let guest_features = device_features & VIRTIO_NET_F_MAC;
        transport.write_guest_features(env, guest_features);

        let mac = [
            transport.read_device_config8(env, 0),
            transport.read_device_config8(env, 1),
            transport.read_device_config8(env, 2),
            transport.read_device_config8(env, 3),
            transport.read_device_config8(env, 4),
            transport.read_device_config8(env, 5),
        ];

        trace!(
            env,
            "MAC address is {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5],
        );

        let txq = transport
            .setup_virtqueue(env, 1)
            .map_err(InitError::TxVqSetup)?;

        let rxq = transport
            .setup_virtqueue(env, 0)
            .map_err(InitError::RxVqSetup)?;

        transport.driver_ok(env);

        Ok(Self {
            mac,
            transport,
            mutable: SpinLock::new(Mutable {
                txq,
                rxq,
                tx_notifier: None,
                rx_notifier: None,
            }),
        })
    }
}

impl<T: VirtioTransport, N: Notifier> Driver for VirtioNet<T, N> {
    type Notifier = N;

    fn mac_address(&self) -> &[u8; 6] {
        &self.mac
    }

    fn try_send(
        &self,
        env: &dyn Env,
        mut header_buf: DmaBuf,
        headroom: usize,
        payload_buf: Option<DmaBuf>,
    ) -> Result<(), (DmaBuf, Option<DmaBuf>, Error)> {
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
        let chain = match payload_buf.as_ref() {
            Some(payload_buf) => {
                &[
                    header_entry,
                    ChainEntry::Read {
                        paddr: payload_buf.paddr() as u64,
                        len: payload_buf.len() as u32,
                    },
                ][..]
            }
            None => &[header_entry],
        };

        if let Err((_, data)) = mutable.txq.push(
            chain,
            TxData {
                header_buf,
                payload_buf,
            },
        ) {
            return Err((data.header_buf, data.payload_buf, Error::TxFull));
        }

        self.transport.notify(env, &mutable.txq);
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

        self.transport.notify(env, &mutable.rxq);
        Ok(())
    }

    fn try_receive(&self, env: &dyn Env) -> Result<(DmaBuf, usize, usize), Error> {
        let mut mutable = self.mutable.lock();
        let (buf, total_len) = match mutable.rxq.pop() {
            Ok(Some((RxData { buf }, total_len))) => (buf, total_len),
            Ok(None) => return Err(Error::RxEmpty),
            Err(err) => {
                trace!(env, "rxq pop error: {err:?}");
                return Err(Error::BadDevice);
            }
        };

        let buf = DmaBufWithDrop::new(env, buf);
        if total_len > buf.len() {
            return Err(Error::BadDevice);
        }

        let header_size = size_of::<VirtioNetHdr>();
        let Some(payload_len) = total_len.checked_sub(header_size) else {
            return Err(Error::BadDevice);
        };

        Ok((buf.take(), header_size, payload_len))
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
        let status = self.transport.read_isr(env);
        if status.virtqueue_updated() {
            loop {
                match mutable.txq.pop() {
                    Ok(Some((data, _total_len))) => {
                        env.free_dma(data.header_buf);
                        if let Some(payload_buf) = data.payload_buf {
                            env.free_dma(payload_buf);
                        }
                    }
                    Ok(None) => break,
                    // Ignore bad descriptors.
                    Err(err) => {
                        warn!(env, "txq pop error: {err:?}");
                        continue;
                    }
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
