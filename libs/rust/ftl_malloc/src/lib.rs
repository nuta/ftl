#![no_std]

use core::mem::MaybeUninit;
use core::mem::size_of;
use core::ptr::NonNull;

use ftl_utils::alignment::align_up;
use ftl_utils::alignment::is_aligned;

const MAGIC_FREE: u32 = 0x5f5f_5f5f;
const MAGIC_ALLOCATED: u32 = 0x5a5a_5a5a;
const HEADER_SIZE: usize = size_of::<Header>();
const MIN_CAPACITY: usize = 16;

/// The minimum size of the data area, and also serves as the alignment of the
/// data area when splitting.
const MIN_SIZE: usize = if cfg!(target_pointer_width = "64") {
    16
} else {
    4
};

/// A header for a memory chunk.
#[repr(C)]
struct Header {
    next: Option<NonNull<Header>>,
    capacity: u32,
    magic: u32,
}

impl Header {
    pub fn data_ptr(&self) -> *mut u8 {
        unsafe { (self as *const Header).cast::<u8>().add(HEADER_SIZE) as *mut u8 }
    }

    unsafe fn from_data_ptr(ptr: *mut u8) -> *mut Header {
        unsafe { ptr.sub(HEADER_SIZE).cast::<Header>() }
    }

    /// Returns a pointer to the data area if this chunk can meet the size/align
    /// requirements.
    pub fn can_fit(&self, size: u32, align: usize) -> Result<*mut u8, ()> {
        let data_ptr = self.data_ptr();
        if self.capacity >= size && is_aligned(data_ptr as usize, align) {
            Ok(data_ptr)
        } else {
            Err(())
        }
    }

    /// Try shrinking the chunk to the given size, and returns an uninitialized
    /// header and the remainder's data capacity if successful.
    pub fn try_split(&mut self, size: u32) -> Option<(*mut MaybeUninit<Header>, u32)> {
        // Check if the chunk is large enough to split.
        let remaining = self.capacity.checked_sub(size)?;
        if remaining < (HEADER_SIZE + MIN_CAPACITY) as u32 {
            return None;
        }

        let remainder = remaining.checked_sub(HEADER_SIZE as u32)?;

        // Shrink this chunk.
        self.capacity = size;

        let new_header_ptr = unsafe { self.data_ptr().add(size as usize) };
        debug_assert!(is_aligned(new_header_ptr as usize, MIN_SIZE));

        Some((new_header_ptr as *mut MaybeUninit<Header>, remainder))
    }
}

/// A K&R malloc implementation.
pub struct LinkedListAllocator {
    head: Option<NonNull<Header>>,
}

impl Default for LinkedListAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self { head: None }
    }

    /// Adds a free memory chunk to the allocator.
    ///
    /// # Safety
    ///
    /// `ptr` must be valid for writes of `size` bytes.
    pub unsafe fn add_chunk(&mut self, ptr: *mut u8, size: usize) {
        if ptr.is_null() || !is_aligned(ptr as usize, MIN_SIZE) {
            return;
        }

        let Some(capacity) = size.checked_sub(HEADER_SIZE) else {
            // Not enough space for a header.
            return;
        };

        if capacity < MIN_CAPACITY {
            // The chunk is too small to be useful.
            return;
        }

        let Ok(capacity) = capacity.try_into() else {
            // The capacity is too large for a u32.
            return;
        };

        unsafe {
            (ptr as *mut Header).write(Header {
                next: self.head,
                capacity,
                magic: MAGIC_FREE,
            });
            self.head = Some(NonNull::new_unchecked(ptr as *mut Header));
        }
    }

    /// Allocates a memory chunk.
    ///
    /// Returns a pointer to the allocated chunk, or `None` if the requested
    /// size/align is not available.
    pub fn malloc(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        debug_assert!(align.is_power_of_two());
        debug_assert!(align > 0);
        debug_assert!(size > 0);

        // Check if the size is suspiciously large.
        if size > u32::MAX as usize / 2 {
            return None;
        }

        // This cast is safe because we checked that the size is under u32::MAX / 2.
        let size = align_up(size, MIN_SIZE) as u32;

        let mut prev: Option<NonNull<Header>> = None;
        let mut next = self.head;
        while let Some(mut head) = next {
            let header: &mut Header = unsafe { head.as_mut() };
            debug_assert_eq!(header.magic, MAGIC_FREE);

            if let Ok(data_ptr) = header.can_fit(size, align) {
                // Split this chunk if it's large enough.
                let next_free = if let Some((new_header, remainder)) = header.try_split(size) {
                    let remainder = unsafe {
                        (*new_header).write(Header {
                            next: header.next,
                            capacity: remainder,
                            magic: MAGIC_FREE,
                        });
                        new_header as *mut Header
                    };
                    Some(unsafe { NonNull::new_unchecked(remainder) })
                } else {
                    header.next
                };

                // Mark this chunk as allocated.
                header.magic = MAGIC_ALLOCATED;
                header.next = None;

                // Remove this chunk from the free list.
                if let Some(mut prev) = prev {
                    unsafe { prev.as_mut() }.next = next_free;
                } else {
                    self.head = next_free;
                }

                return Some(data_ptr);
            }

            prev = Some(head);
            next = header.next;
        }

        None
    }

    /// Releases a pointer returned by `malloc`.
    ///
    /// # Safety
    ///
    /// `ptr` must have been returned by a previous successful call to
    /// `malloc` on this allocator and must not have already been freed.
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        let mut header_ptr = unsafe { NonNull::new_unchecked(Header::from_data_ptr(ptr)) };
        let header = unsafe { header_ptr.as_mut() };
        debug_assert_eq!(header.magic, MAGIC_ALLOCATED);

        header.magic = MAGIC_FREE;
        header.next = self.head;
        self.head = Some(header_ptr);
    }
}

unsafe impl Send for LinkedListAllocator {}
