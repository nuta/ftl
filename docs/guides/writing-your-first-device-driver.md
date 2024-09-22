---
title: Writing Your First Device Driver
---

## Scaffolding

## Look for an interface in IDL

## Goldfish RTC device 101

[specification](https://android.googlesource.com/platform/external/qemu/+/refs/heads/emu-2.0-release/docs/GOLDFISH-VIRTUAL-HARDWARE.TXT#:~:text=Goldfish%20real%2Dtime%20clock).

## Mapping MMIO registers

## Allocating DMA memory

Use `MappedFolio::create` to allocate a physically-contiguous memory block, and  `ftl_driver_utils::buffer_pool::BufferPool` to manage the allocated DMA space.

## Handling hardware interrupts

## Next Steps
