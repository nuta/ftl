mod fork;
mod spec;

use alloc::vec;
use alloc::vec::Vec;

pub use fork::Fork;
use ftl::handle::OwnedHandle;
use ftl::hspace::HandleSpace;
use ftl::quota::Quota;
use ftl::thread::Thread;
use ftl::vmo::Vmo;
use ftl::vmspace::VmSpace;
use ftl_elf::Elf;
use ftl_elf::PhdrType;
use ftl_types::handle::HandleId;
use ftl_types::handle::HandleRight;
use ftl_types::quota::Resource;
use ftl_types::quota::UNLIMITED;
use ftl_types::vmspace::PageAttrs;
use ftl_utils::alignment::align_down;
use ftl_utils::alignment::align_up;

use crate::types::errno::Errno;
use crate::vm::PAGE_SIZE;
use crate::zone::spec::NetSpec;
use crate::zone::spec::Spec;

// FIXME: Remove these hard-coded parameters.
const STACK_SIZE: usize = 256 * 1024;
const STACK_TOP: usize = 0x40000000;

fn load_lx_elf(quota: &Quota, vmspace: &VmSpace, image: &Vmo) -> Result<usize, Errno> {
    let mut header = vec![0u8; PAGE_SIZE];
    image.read(0, &mut header)?;
    let elf = Elf::parse(&header, ftl_elf::ET_EXEC).map_err(|_| Errno::ENOEXEC)?;

    for phdr in elf.phdrs {
        if phdr.p_type != PhdrType::Load as u32 || phdr.p_memsz == 0 {
            continue;
        }

        let region_len = align_up(phdr.p_memsz as usize, PAGE_SIZE);
        let attrs = PageAttrs::from_phdr_flags(phdr.p_flags);
        if phdr.writable() {
            let vmo = image.snapshot(quota, phdr.p_offset as usize, region_len)?;
            vmspace.map(&vmo, phdr.p_vaddr as usize, 0, region_len, attrs)?;
        } else {
            vmspace.map(
                image,
                phdr.p_vaddr as usize,
                phdr.p_offset as usize,
                region_len,
                attrs,
            )?;
        }
    }

    Ok(elf.ehdr.e_entry as usize)
}

fn prepare_stack(quota: &Quota, vmspace: &VmSpace, cmdline: &[u8]) -> Result<usize, Errno> {
    if cmdline.len() > STACK_SIZE / 2 {
        return Err(Errno::E2BIG);
    }

    let vmo = Vmo::create_zeroed(quota, STACK_SIZE)?;
    let stack_bottom = STACK_TOP - STACK_SIZE;
    let cmdline_offset = STACK_SIZE - cmdline.len();
    let sp_offset = align_down(cmdline_offset - 2 * size_of::<usize>(), 16);
    let cmdline_ptr = stack_bottom + cmdline_offset;

    vmo.write(cmdline_offset, cmdline)?;
    vmo.write(sp_offset, &cmdline_ptr.to_ne_bytes())?;
    vmo.write(sp_offset + 8, &cmdline.len().to_ne_bytes())?;

    vmspace.map(
        &vmo,
        stack_bottom,
        0,
        STACK_SIZE,
        PageAttrs::READ | PageAttrs::WRITE,
    )?;
    Ok(stack_bottom + sp_offset)
}

fn move_handle_to(
    hspace: &HandleSpace,
    handle: OwnedHandle,
    rights: HandleRight,
    expected: usize,
) -> Result<(), Errno> {
    let id = hspace
        .move_handle(handle, rights)
        .map_err(|(err, _handle)| err)?;

    if id != HandleId::new(expected) {
        return Err(Errno::EEXIST);
    }

    Ok(())
}

pub struct Zone {
    _hspace: HandleSpace,
    _thread: Thread,
}

impl Zone {
    pub fn spawn(parent_quota: &Quota, image: &Vmo, spec: &Spec) -> Result<Zone, Errno> {
        let mut cmdline = Vec::new();
        cmdline.extend_from_slice(b"ftl.lx.console=none ftl.lx.net=none ftl.lx.init=");
        cmdline.extend_from_slice(&spec.init);

        if !matches!(spec.net, NetSpec::None) {
            // TODO:
            return Err(Errno::EINVAL);
        }

        let quota = Quota::create(parent_quota)?;
        quota.limit(Resource::Memory, UNLIMITED)?;
        quota.limit(Resource::CpuTime, UNLIMITED)?;

        let hspace = HandleSpace::create()?;
        let vmspace = VmSpace::create(&quota)?;
        let entry = load_lx_elf(&quota, &vmspace, image)?;
        let sp = prepare_stack(&quota, &vmspace, &cmdline)?;
        let child_image = image.duplicate(HandleRight::READ)?;
        let child_hspace = hspace.duplicate(HandleRight::WRITE)?;
        let thread = Thread::create(&quota, &hspace, &vmspace, 0, 0, 0)?;

        move_handle_to(
            &hspace,
            vmspace.into_handle(),
            HandleRight::READ | HandleRight::WRITE,
            1,
        )?;
        move_handle_to(&hspace, quota.into_handle(), HandleRight::CHARGE, 2)?;
        move_handle_to(&hspace, child_image.into_handle(), HandleRight::READ, 3)?;
        move_handle_to(&hspace, child_hspace.into_handle(), HandleRight::WRITE, 4)?;

        thread.start(entry, sp)?;
        Ok(Zone {
            _hspace: hspace,
            _thread: thread,
        })
    }
}
