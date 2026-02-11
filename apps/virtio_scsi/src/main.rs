#![no_std]
#![no_main]

use core::cmp::min;
use core::ptr::copy_nonoverlapping;
use core::ptr::write_bytes;
use core::slice;

use ftl::application::Application;
use ftl::application::Context;
use ftl::application::InitContext;
use ftl::error::ErrorCode;
use ftl::interrupt::Interrupt;
use ftl::log::*;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl_virtio::ChainEntry;
use ftl_virtio::VirtQueue;
use ftl_virtio::VirtioPci;
use ftl_virtio::virtio_pci::DeviceType;
use ftl_virtio::virtqueue;

const REQUEST_QUEUE_INDEX: u16 = 2;
const EVENT_QUEUE_INDEX: u16 = 1;
const EVENT_BUFFER_LEN: usize = 16;

const SCSI_REQ_LEN: usize = 8 + 8 + 1 + 1 + 1 + 32;
const SCSI_RESP_LEN: usize = 4 + 4 + 2 + 1 + 1 + 96;

const INQUIRY_ALLOC_LEN: usize = 96;
const READ_CAPACITY10_ALLOC_LEN: usize = 8;

const MAX_SCAN_TARGET: u16 = 15;

const OPCODE_INQUIRY: u8 = 0x12;
const OPCODE_TEST_UNIT_READY: u8 = 0x00;
const OPCODE_READ_CAPACITY10: u8 = 0x25;
const OPCODE_READ10: u8 = 0x28;
const OPCODE_WRITE10: u8 = 0x2a;
const OPCODE_SYNCHRONIZE_CACHE10: u8 = 0x35;

const VIRTIO_SCSI_S_OK: u8 = 0;
const SCSI_STATUS_GOOD: u8 = 0;

const JOKE_PAYLOAD: &[u8] =
    b"Joke 1: Why did the kernel panic? It could not handle the pressure.\n\
Joke 2: There are 10 kinds of people: those who understand binary and those who do not.\n\
Joke 3: I would tell you a UDP joke, but you might not get it.\n";

#[derive(Debug)]
enum DriverError {
    DmaBufAlloc(ErrorCode),
    VirtQueueFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataDirection {
    None,
    DeviceToDriver,
    DriverToDevice,
}

#[derive(Clone, Copy, Debug)]
enum RequestKind {
    Inquiry {
        target: u16,
        lun: u16,
    },
    TestUnitReady {
        target: u16,
        lun: u16,
        retries: u8,
    },
    ReadCapacity10 {
        target: u16,
        lun: u16,
        retries: u8,
    },
    Write10 {
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
    },
    SynchronizeCache10 {
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
    },
    Read10 {
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
    },
}

struct OngoingRequest {
    kind: RequestKind,
    data_direction: DataDirection,
    data_vaddr: usize,
    data_len: usize,
    resp_vaddr: usize,
}

struct OngoingEvent {
    vaddr: usize,
    paddr: usize,
}

struct DiskRoundtripTest {
    target: u16,
    lun: u16,
    lba: u32,
    transfer_blocks: u16,
    payload_len: usize,
    expected: Vec<u8>,
}

struct Main {
    virtio: VirtioPci,
    eventq: VirtQueue,
    requestq: VirtQueue,
    ongoing_events: Vec<Option<OngoingEvent>>,
    ongoing: Vec<Option<OngoingRequest>>,
    next_tag: u64,
    scan_next_target: u16,
    max_scan_target: u16,
    found_disks: usize,
    disk_test: Option<DiskRoundtripTest>,
    disk_test_done: bool,
}

impl Application for Main {
    fn init(ctx: &mut InitContext) -> Self {
        trace!("virtio-scsi app starting...");

        let prober = VirtioPci::probe(DeviceType::Scsi).unwrap();
        let device_features = prober.read_guest_features();
        let guest_features = 0;
        let (virtio, interrupt) = prober.finish(guest_features);

        ctx.add_interrupt(Rc::new(interrupt)).unwrap();

        let mut eventq = virtio.setup_virtqueue(EVENT_QUEUE_INDEX).unwrap();
        let requestq = virtio.setup_virtqueue(REQUEST_QUEUE_INDEX).unwrap();

        let mut ongoing_events = Vec::with_capacity(eventq.queue_size());
        for _ in 0..eventq.queue_size() {
            ongoing_events.push(None);
        }

        for _ in 0..min(eventq.queue_size(), 4) {
            let (vaddr, paddr) = Self::alloc_dma(EVENT_BUFFER_LEN).unwrap();
            unsafe {
                write_bytes(vaddr as *mut u8, 0, EVENT_BUFFER_LEN);
            }
            let head = eventq
                .push(&[ChainEntry::Write {
                    paddr: paddr as u64,
                    len: EVENT_BUFFER_LEN as u32,
                }])
                .unwrap();
            ongoing_events[head.0 as usize] = Some(OngoingEvent { vaddr, paddr });
        }
        virtio.notify(&eventq);

        let mut ongoing = Vec::with_capacity(requestq.queue_size());
        for _ in 0..requestq.queue_size() {
            ongoing.push(None);
        }

        let config_max_target = read_config_u16(&virtio, 30);
        let max_scan_target = min(min(config_max_target, 0xff), MAX_SCAN_TARGET);

        trace!(
            "virtio-scsi config: features=0x{:08x}, max_target={} (scanning target IDs 0..={})",
            device_features, config_max_target, max_scan_target
        );

        let mut app = Self {
            virtio,
            eventq,
            requestq,
            ongoing_events,
            ongoing,
            next_tag: 1,
            scan_next_target: 0,
            max_scan_target,
            found_disks: 0,
            disk_test: None,
            disk_test_done: false,
        };

        app.queue_next_inquiry();

        app
    }

    fn irq(&mut self, _ctx: &mut Context, interrupt: &Rc<Interrupt>, _irq: u8) {
        let isr = self.virtio.read_isr();
        if isr.virtqueue_updated() {
            while let Some(used) = self.eventq.pop() {
                self.handle_event(used.head.0 as usize);
            }
            while let Some(used) = self.requestq.pop() {
                self.handle_completion(used.head.0 as usize);
            }
        }

        interrupt.acknowledge().unwrap();
    }
}

impl Main {
    fn alloc_dma(size: usize) -> Result<(usize, usize), DriverError> {
        let mut vaddr = 0usize;
        let mut paddr = 0usize;
        ftl::dmabuf::sys_dmabuf_alloc(size.max(4096), &mut vaddr, &mut paddr)
            .map_err(DriverError::DmaBufAlloc)?;
        Ok((vaddr, paddr))
    }

    fn queue_next_inquiry(&mut self) {
        if self.scan_next_target > self.max_scan_target {
            trace!(
                "virtio-scsi scan complete: found {} disk(s)",
                self.found_disks
            );
            return;
        }

        let target = self.scan_next_target;
        self.scan_next_target = self.scan_next_target.saturating_add(1);

        if let Err(error) = self.submit_inquiry(target, 0) {
            trace!(
                "failed to submit INQUIRY for target {}: {:?}",
                target, error
            );
            self.queue_next_inquiry();
        }
    }

    fn submit_inquiry(&mut self, target: u16, lun: u16) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_INQUIRY;
        cdb[4] = INQUIRY_ALLOC_LEN as u8;

        self.submit_scsi_command(
            RequestKind::Inquiry { target, lun },
            encode_lun(target, lun),
            cdb,
            DataDirection::DeviceToDriver,
            INQUIRY_ALLOC_LEN,
            None,
        )
    }

    fn submit_read_capacity10(&mut self, target: u16, lun: u16) -> Result<(), DriverError> {
        self.submit_read_capacity10_with_retry(target, lun, 0)
    }

    fn submit_read_capacity10_with_retry(
        &mut self,
        target: u16,
        lun: u16,
        retries: u8,
    ) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_READ_CAPACITY10;

        self.submit_scsi_command(
            RequestKind::ReadCapacity10 {
                target,
                lun,
                retries,
            },
            encode_lun(target, lun),
            cdb,
            DataDirection::DeviceToDriver,
            READ_CAPACITY10_ALLOC_LEN,
            None,
        )
    }

    fn submit_test_unit_ready(&mut self, target: u16, lun: u16) -> Result<(), DriverError> {
        self.submit_test_unit_ready_with_retry(target, lun, 0)
    }

    fn submit_test_unit_ready_with_retry(
        &mut self,
        target: u16,
        lun: u16,
        retries: u8,
    ) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_TEST_UNIT_READY;

        self.submit_scsi_command(
            RequestKind::TestUnitReady {
                target,
                lun,
                retries,
            },
            encode_lun(target, lun),
            cdb,
            DataDirection::None,
            0,
            None,
        )
    }

    fn submit_write10(
        &mut self,
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
        payload: &[u8],
    ) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_WRITE10;
        cdb[2..6].copy_from_slice(&lba.to_be_bytes());
        cdb[7..9].copy_from_slice(&transfer_blocks.to_be_bytes());

        self.submit_scsi_command(
            RequestKind::Write10 {
                target,
                lun,
                lba,
                transfer_blocks,
            },
            encode_lun(target, lun),
            cdb,
            DataDirection::DriverToDevice,
            payload.len(),
            Some(payload),
        )
    }

    fn submit_synchronize_cache10(
        &mut self,
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
    ) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_SYNCHRONIZE_CACHE10;
        cdb[2..6].copy_from_slice(&lba.to_be_bytes());
        cdb[7..9].copy_from_slice(&transfer_blocks.to_be_bytes());

        self.submit_scsi_command(
            RequestKind::SynchronizeCache10 {
                target,
                lun,
                lba,
                transfer_blocks,
            },
            encode_lun(target, lun),
            cdb,
            DataDirection::None,
            0,
            None,
        )
    }

    fn submit_read10(
        &mut self,
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
        data_len: usize,
    ) -> Result<(), DriverError> {
        let mut cdb = [0u8; 32];
        cdb[0] = OPCODE_READ10;
        cdb[2..6].copy_from_slice(&lba.to_be_bytes());
        cdb[7..9].copy_from_slice(&transfer_blocks.to_be_bytes());

        self.submit_scsi_command(
            RequestKind::Read10 {
                target,
                lun,
                lba,
                transfer_blocks,
            },
            encode_lun(target, lun),
            cdb,
            DataDirection::DeviceToDriver,
            data_len,
            None,
        )
    }

    fn submit_scsi_command(
        &mut self,
        kind: RequestKind,
        lun: [u8; 8],
        cdb: [u8; 32],
        data_direction: DataDirection,
        data_len: usize,
        data_out: Option<&[u8]>,
    ) -> Result<(), DriverError> {
        let (req_vaddr, req_paddr) = Self::alloc_dma(SCSI_REQ_LEN)?;
        let (resp_vaddr, resp_paddr) = Self::alloc_dma(SCSI_RESP_LEN)?;
        let (data_vaddr, data_paddr) = if data_len > 0 {
            Self::alloc_dma(data_len)?
        } else {
            (0, 0)
        };

        // Request layout: lun[8], tag[8], task_attr[1], prio[1], crn[1], cdb[32].
        let mut req = [0u8; SCSI_REQ_LEN];
        req[0..8].copy_from_slice(&lun);
        req[8..16].copy_from_slice(&self.next_tag.to_le_bytes());
        req[19..(19 + cdb.len())].copy_from_slice(&cdb);
        self.next_tag = self.next_tag.wrapping_add(1);

        unsafe {
            copy_nonoverlapping(req.as_ptr(), req_vaddr as *mut u8, req.len());
            write_bytes(resp_vaddr as *mut u8, 0, SCSI_RESP_LEN);
            match data_direction {
                DataDirection::None => {}
                DataDirection::DeviceToDriver => {
                    assert!(data_len > 0);
                    write_bytes(data_vaddr as *mut u8, 0, data_len);
                }
                DataDirection::DriverToDevice => {
                    let payload = data_out.expect("missing SCSI data-out payload");
                    assert_eq!(payload.len(), data_len);
                    copy_nonoverlapping(payload.as_ptr(), data_vaddr as *mut u8, payload.len());
                }
            }
        }

        let head = match data_direction {
            DataDirection::None => {
                let chain = [
                    ChainEntry::Read {
                        paddr: req_paddr as u64,
                        len: SCSI_REQ_LEN as u32,
                    },
                    ChainEntry::Write {
                        paddr: resp_paddr as u64,
                        len: SCSI_RESP_LEN as u32,
                    },
                ];
                self.requestq.push(&chain)
            }
            DataDirection::DeviceToDriver => {
                let chain = [
                    ChainEntry::Read {
                        paddr: req_paddr as u64,
                        len: SCSI_REQ_LEN as u32,
                    },
                    ChainEntry::Write {
                        paddr: resp_paddr as u64,
                        len: SCSI_RESP_LEN as u32,
                    },
                    ChainEntry::Write {
                        paddr: data_paddr as u64,
                        len: data_len as u32,
                    },
                ];
                self.requestq.push(&chain)
            }
            DataDirection::DriverToDevice => {
                let chain = [
                    ChainEntry::Read {
                        paddr: req_paddr as u64,
                        len: SCSI_REQ_LEN as u32,
                    },
                    ChainEntry::Read {
                        paddr: data_paddr as u64,
                        len: data_len as u32,
                    },
                    ChainEntry::Write {
                        paddr: resp_paddr as u64,
                        len: SCSI_RESP_LEN as u32,
                    },
                ];
                self.requestq.push(&chain)
            }
        }
        .map_err(|virtqueue::FullError| DriverError::VirtQueueFull)?;

        self.ongoing[head.0 as usize] = Some(OngoingRequest {
            kind,
            data_direction,
            data_vaddr,
            data_len,
            resp_vaddr,
        });

        self.virtio.notify(&self.requestq);

        Ok(())
    }

    fn handle_completion(&mut self, head_index: usize) {
        let Some(request) = self.ongoing[head_index].take() else {
            trace!("missing request for descriptor head {}", head_index);
            return;
        };

        let resp = unsafe { slice::from_raw_parts(request.resp_vaddr as *const u8, SCSI_RESP_LEN) };
        let response = resp[11];
        let status = resp[10];
        let resid = read_le_u32(&resp[4..8]);
        let sense_len = min(read_le_u32(&resp[0..4]) as usize, 96);
        let sense = &resp[12..(12 + sense_len)];

        if response != VIRTIO_SCSI_S_OK || status != SCSI_STATUS_GOOD {
            match request.kind {
                RequestKind::TestUnitReady {
                    target,
                    lun,
                    retries,
                } => {
                    if retries < 3 && is_retryable_check_condition(response, status, sense) {
                        trace!(
                            "TEST UNIT READY retry {}/3 for target {} lun {}",
                            retries + 1,
                            target,
                            lun
                        );
                        if let Err(error) =
                            self.submit_test_unit_ready_with_retry(target, lun, retries + 1)
                        {
                            trace!(
                                "failed to resubmit TEST UNIT READY for target {} lun {}: {:?}",
                                target, lun, error
                            );
                            self.queue_next_inquiry();
                        }
                        return;
                    }
                }
                RequestKind::ReadCapacity10 {
                    target,
                    lun,
                    retries,
                } => {
                    if retries < 3 && is_retryable_check_condition(response, status, sense) {
                        trace!(
                            "READ CAPACITY(10) retry {}/3 for target {} lun {}",
                            retries + 1,
                            target,
                            lun
                        );
                        if let Err(error) =
                            self.submit_read_capacity10_with_retry(target, lun, retries + 1)
                        {
                            trace!(
                                "failed to resubmit READ CAPACITY(10) for target {} lun {}: {:?}",
                                target, lun, error
                            );
                            self.queue_next_inquiry();
                        }
                        return;
                    }
                }
                RequestKind::Inquiry { .. } => {}
                RequestKind::Write10 { .. } => {
                    self.disk_test = None;
                    self.disk_test_done = true;
                }
                RequestKind::SynchronizeCache10 { .. } => {
                    self.disk_test = None;
                    self.disk_test_done = true;
                }
                RequestKind::Read10 { .. } => {
                    self.disk_test = None;
                    self.disk_test_done = true;
                }
            }

            trace!(
                "SCSI {:?} failed: response={}, status=0x{:02x}, resid={}, sense_len={}",
                request.kind, response, status, resid, sense_len
            );
            self.queue_next_inquiry();
            return;
        }

        let data =
            if request.data_direction == DataDirection::DeviceToDriver && request.data_len > 0 {
                unsafe { slice::from_raw_parts(request.data_vaddr as *const u8, request.data_len) }
            } else {
                &[]
            };

        match request.kind {
            RequestKind::Inquiry { target, lun } => {
                self.handle_inquiry_success(target, lun, data);
            }
            RequestKind::TestUnitReady {
                target,
                lun,
                retries: _,
            } => {
                self.handle_test_unit_ready_success(target, lun);
            }
            RequestKind::ReadCapacity10 {
                target,
                lun,
                retries: _,
            } => {
                self.handle_read_capacity_success(target, lun, data);
            }
            RequestKind::Write10 {
                target,
                lun,
                lba,
                transfer_blocks,
            } => {
                self.handle_write10_success(target, lun, lba, transfer_blocks);
            }
            RequestKind::SynchronizeCache10 {
                target,
                lun,
                lba,
                transfer_blocks,
            } => {
                self.handle_synchronize_cache10_success(target, lun, lba, transfer_blocks);
            }
            RequestKind::Read10 {
                target,
                lun,
                lba,
                transfer_blocks,
            } => {
                self.handle_read10_success(target, lun, lba, transfer_blocks, data);
            }
        }
    }

    fn handle_event(&mut self, head_index: usize) {
        let Some(event) = self.ongoing_events[head_index].take() else {
            trace!("missing virtio-scsi event buffer for head {}", head_index);
            return;
        };

        let raw = unsafe { slice::from_raw_parts(event.vaddr as *const u8, EVENT_BUFFER_LEN) };
        let event_code = read_le_u32(&raw[0..4]);
        let reason = read_le_u32(&raw[12..16]);
        trace!(
            "virtio-scsi event: code=0x{:08x}, reason=0x{:08x}",
            event_code, reason
        );

        unsafe {
            write_bytes(event.vaddr as *mut u8, 0, EVENT_BUFFER_LEN);
        }

        let head = match self.eventq.push(&[ChainEntry::Write {
            paddr: event.paddr as u64,
            len: EVENT_BUFFER_LEN as u32,
        }]) {
            Ok(head) => head,
            Err(virtqueue::FullError) => {
                trace!("virtio-scsi event queue is full while requeueing");
                return;
            }
        };

        self.ongoing_events[head.0 as usize] = Some(event);
        self.virtio.notify(&self.eventq);
    }

    fn handle_inquiry_success(&mut self, target: u16, lun: u16, data: &[u8]) {
        if data.len() < 36 {
            trace!(
                "INQUIRY response too short for target {} lun {}: {} bytes",
                target,
                lun,
                data.len()
            );
            self.queue_next_inquiry();
            return;
        }

        let qualifier = data[0] >> 5;
        let peripheral = data[0] & 0x1f;

        if qualifier == 0x3 || peripheral == 0x1f {
            trace!("target {} lun {}: no device", target, lun);
            self.queue_next_inquiry();
            return;
        }

        let vendor = ascii_field(&data[8..16]);
        let product = ascii_field(&data[16..32]);
        trace!(
            "target {} lun {}: peripheral=0x{:02x}, vendor='{}', product='{}'",
            target, lun, peripheral, vendor, product
        );

        self.found_disks += 1;

        if let Err(error) = self.submit_test_unit_ready(target, lun) {
            trace!(
                "failed to submit TEST UNIT READY for target {} lun {}: {:?}",
                target, lun, error
            );
            self.queue_next_inquiry();
        }
    }

    fn handle_test_unit_ready_success(&mut self, target: u16, lun: u16) {
        if let Err(error) = self.submit_read_capacity10(target, lun) {
            trace!(
                "failed to submit READ CAPACITY(10) for target {} lun {}: {:?}",
                target, lun, error
            );
            self.queue_next_inquiry();
        }
    }

    fn handle_read_capacity_success(&mut self, target: u16, lun: u16, data: &[u8]) {
        if data.len() < READ_CAPACITY10_ALLOC_LEN {
            trace!(
                "READ CAPACITY(10) response too short for target {} lun {}: {} bytes",
                target,
                lun,
                data.len()
            );
            self.queue_next_inquiry();
            return;
        }

        let last_lba = read_be_u32(&data[0..4]);
        let block_len = read_be_u32(&data[4..8]);

        if block_len == 0 {
            trace!(
                "target {} lun {}: invalid block size reported (0 bytes)",
                target, lun
            );
            self.queue_next_inquiry();
            return;
        }

        if last_lba == u32::MAX {
            trace!(
                "target {} lun {}: READ CAPACITY(10) overflow, disk requires READ CAPACITY(16)",
                target, lun
            );
            self.queue_next_inquiry();
            return;
        }

        let capacity_bytes = (u64::from(last_lba) + 1) * u64::from(block_len);
        let capacity_mib = capacity_bytes / (1024 * 1024);

        trace!(
            "target {} lun {} capacity: {} bytes ({} MiB, block={} bytes)",
            target, lun, capacity_bytes, capacity_mib, block_len
        );

        if !self.disk_test_done && self.disk_test.is_none() {
            self.start_disk_roundtrip_test(target, lun, last_lba, block_len);
            return;
        }

        self.queue_next_inquiry();
    }

    fn start_disk_roundtrip_test(&mut self, target: u16, lun: u16, last_lba: u32, block_len: u32) {
        let block_len = block_len as usize;
        if block_len == 0 {
            trace!("skip disk roundtrip test: block size is zero");
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        let required_blocks = JOKE_PAYLOAD.len().div_ceil(block_len);
        if required_blocks == 0 || required_blocks > usize::from(u16::MAX) {
            trace!(
                "skip disk roundtrip test: payload requires invalid block count ({})",
                required_blocks
            );
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        let total_blocks = last_lba as usize + 1;
        if required_blocks > total_blocks {
            trace!(
                "skip disk roundtrip test: disk too small (need {} blocks, have {})",
                required_blocks, total_blocks
            );
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        let transfer_blocks = required_blocks as u16;
        let lba: u32 = if total_blocks > required_blocks { 1 } else { 0 };
        let total_data_len = required_blocks * block_len;

        let mut expected = Vec::with_capacity(total_data_len);
        expected.resize(total_data_len, 0);
        expected[..JOKE_PAYLOAD.len()].copy_from_slice(JOKE_PAYLOAD);

        trace!(
            "disk roundtrip test: writing {} bytes ({} blocks) at target {} lun {} lba {}",
            JOKE_PAYLOAD.len(),
            transfer_blocks,
            target,
            lun,
            lba
        );

        if let Err(error) = self.submit_write10(target, lun, lba, transfer_blocks, &expected) {
            trace!(
                "failed to submit WRITE(10) for target {} lun {}: {:?}",
                target, lun, error
            );
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        self.disk_test = Some(DiskRoundtripTest {
            target,
            lun,
            lba,
            transfer_blocks,
            payload_len: JOKE_PAYLOAD.len(),
            expected,
        });
    }

    fn handle_write10_success(&mut self, target: u16, lun: u16, lba: u32, transfer_blocks: u16) {
        trace!(
            "WRITE(10) succeeded for target {} lun {} at lba {} ({} blocks), flushing cache",
            target, lun, lba, transfer_blocks
        );

        if let Err(error) = self.submit_synchronize_cache10(target, lun, lba, transfer_blocks) {
            trace!(
                "failed to submit SYNCHRONIZE CACHE(10) for target {} lun {}: {:?}",
                target, lun, error
            );
            self.disk_test = None;
            self.disk_test_done = true;
            self.queue_next_inquiry();
        }
    }

    fn handle_synchronize_cache10_success(
        &mut self,
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
    ) {
        trace!(
            "SYNCHRONIZE CACHE(10) succeeded for target {} lun {}, reading data back",
            target, lun
        );

        let Some(test) = self.disk_test.as_ref() else {
            trace!("missing disk roundtrip state before READ(10)");
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        };

        if test.target != target
            || test.lun != lun
            || test.lba != lba
            || test.transfer_blocks != transfer_blocks
        {
            trace!("disk roundtrip state mismatch before READ(10)");
            self.disk_test = None;
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        if let Err(error) =
            self.submit_read10(target, lun, lba, transfer_blocks, test.expected.len())
        {
            trace!(
                "failed to submit READ(10) for target {} lun {}: {:?}",
                target, lun, error
            );
            self.disk_test = None;
            self.disk_test_done = true;
            self.queue_next_inquiry();
        }
    }

    fn handle_read10_success(
        &mut self,
        target: u16,
        lun: u16,
        lba: u32,
        transfer_blocks: u16,
        data: &[u8],
    ) {
        let Some(test) = self.disk_test.take() else {
            trace!("READ(10) completed without an active disk roundtrip test");
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        };

        if test.target != target
            || test.lun != lun
            || test.lba != lba
            || test.transfer_blocks != transfer_blocks
        {
            trace!("READ(10) completed for unexpected target/lun/lba");
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        if data.len() < test.expected.len() {
            trace!(
                "READ(10) data too short: expected {} bytes, got {} bytes",
                test.expected.len(),
                data.len()
            );
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        let read_back = &data[..test.expected.len()];
        if read_back != test.expected.as_slice() {
            let mismatch = first_mismatch_offset(read_back, test.expected.as_slice()).unwrap_or(0);
            trace!(
                "disk roundtrip mismatch at byte {}: wrote=0x{:02x}, read=0x{:02x}",
                mismatch, test.expected[mismatch], read_back[mismatch]
            );
            self.disk_test_done = true;
            self.queue_next_inquiry();
            return;
        }

        let message = match core::str::from_utf8(&read_back[..test.payload_len]) {
            Ok(text) => text,
            Err(_) => "<invalid utf-8>",
        };

        trace!(
            "disk roundtrip verified at target {} lun {} lba {} ({} blocks)",
            target, lun, lba, transfer_blocks
        );
        trace!("disk payload:\n{}", message);

        self.disk_test_done = true;
        self.queue_next_inquiry();
    }
}

fn read_config_u16(virtio: &VirtioPci, offset: u16) -> u16 {
    let lo = virtio.read_device_config8(offset);
    let hi = virtio.read_device_config8(offset + 1);
    u16::from(lo) | (u16::from(hi) << 8)
}

fn encode_lun(target: u16, lun: u16) -> [u8; 8] {
    assert!(target <= 0xff);
    assert!(lun <= 0x3fff);

    // Virtio-SCSI transport LUN format used by Linux/QEMU.
    let mut encoded = [0u8; 8];
    encoded[0] = 1;
    encoded[1] = target as u8;
    encoded[2] = ((lun >> 8) as u8) | 0x40;
    encoded[3] = lun as u8;
    encoded
}

fn read_le_u32(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]))
        | (u32::from(bytes[1]) << 8)
        | (u32::from(bytes[2]) << 16)
        | (u32::from(bytes[3]) << 24)
}

fn read_be_u32(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]) << 24)
        | (u32::from(bytes[1]) << 16)
        | (u32::from(bytes[2]) << 8)
        | (u32::from(bytes[3]))
}

fn first_mismatch_offset(lhs: &[u8], rhs: &[u8]) -> Option<usize> {
    let len = min(lhs.len(), rhs.len());
    for i in 0..len {
        if lhs[i] != rhs[i] {
            return Some(i);
        }
    }
    if lhs.len() == rhs.len() {
        None
    } else {
        Some(len)
    }
}

fn ascii_field(bytes: &[u8]) -> &str {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }

    match core::str::from_utf8(&bytes[..end]) {
        Ok(value) => value,
        Err(_) => "<invalid-utf8>",
    }
}

fn is_retryable_check_condition(response: u8, status: u8, sense: &[u8]) -> bool {
    if response != VIRTIO_SCSI_S_OK || status != 0x02 {
        return false;
    }
    if sense.len() < 3 {
        return false;
    }

    let sense_key = sense[2] & 0x0f;
    // NOT READY or UNIT ATTENTION
    sense_key == 0x02 || sense_key == 0x06
}

#[unsafe(no_mangle)]
fn main() {
    ftl::application::run::<Main>();
}
