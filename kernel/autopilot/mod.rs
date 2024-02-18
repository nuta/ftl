use alloc::collections::BTreeMap;
use alloc::ffi::CString;
use alloc::string::ToString;
use alloc::vec::Vec;

use ftl_types::environ::Environ;
use ftl_types::spec::DeviceTreeEntry;
use ftl_types::spec::FiberSpec;

use crate::boot::BootInfo;
use crate::fiber::Fiber;

mod device_tree;

struct Autopilot {
    devices: Vec<device_tree::Device>,
}

impl Autopilot {
    pub fn new(bootinfo: &BootInfo) -> Autopilot {
        let devices = device_tree::walk_device_nodes(bootinfo.dtb_addr);
        Autopilot { devices }
    }

    fn visit_fiber(&mut self, spec: &FiberSpec, main: fn(*const i8)) {
        let device = match &spec.device_tree {
            Some(entries) => {
                match self.find_device_for_fiber(entries) {
                    Some(device) => Some(device),
                    None => {
                        println!("device not found for fiber {:?}", spec.name);
                        return;
                    }
                }
            }
            None => {
                todo!()
            }
        };

        let environ_device = device.map(|device| {
            let interrupts = match &device.interrupts {
                Some(interrupts) => {
                    let mut vec = Vec::new();
                    for interrupt in interrupts {
                        vec.push(*interrupt);
                    }
                    Some(vec)
                }
                None => None,
            };

            ftl_types::environ::Device {
                name: device.name.to_string(),
                compatible: device.compatible.to_string(),
                reg: device.reg,
                interrupts,
            }
        });

        let deps = BTreeMap::new();
        for dep_name in spec.deps.iter() {
            if dep_name == "autopilot" {
                //
            }
        }

        println!("autopilot: starting {}", spec.name);
        let fiber = Fiber::new();
        let environ = Environ {
            deps,
            device: environ_device,
        };
        let environ_json = serde_json::to_string(&environ).unwrap();
        let environ_cstr = CString::new(environ_json).unwrap();

        fiber.spawn_in_kernel(move || {
            main(environ_cstr.as_ptr());
        });
    }

    fn visit_kernel_fibers(&mut self, bootinfo: &BootInfo) {
        for (spec, main) in bootinfo.kernel_fibers.iter() {
            let spec: FiberSpec =
                serde_json::from_str(spec).expect("failed to parse an in-kernel fiber spec");

            self.visit_fiber(&spec, *main);
        }
    }

    fn find_device_for_fiber(&self, patterns: &[DeviceTreeEntry]) -> Option<&device_tree::Device> {
        for pattern in patterns {
            for device in self.devices.iter() {
                if device.compatible == pattern.compatible {
                    return Some(device);
                }
            }
        }

        None
    }
}

pub fn start(bootinfo: &BootInfo) {
    let devices = device_tree::walk_device_nodes(bootinfo.dtb_addr);
    println!("device tree: found {} devices", devices.len());
    for device in devices {
        println!(
            "device tree: {} (compatible \"{}\")",
            device.name, device.compatible
        );
    }

    // let (ping_ch, pong_ch) = Channel::new().unwrap();
    // let mut ping_ch = Some(ping_ch);
    // let mut pong_ch = Some(pong_ch);
    // if spec.name == "ping" {
    //     deps.insert("pong".to_string(), handle);
    //     fiber.insert_handle(
    //         handle,
    //         crate::fiber::Object::Channel(ping_ch.take().unwrap()),
    //     );
    // } else if spec.name == "pong" {
    //     deps.insert("ping".to_string(), handle);
    //     fiber.insert_handle(
    //         handle,
    //         crate::fiber::Object::Channel(pong_ch.take().unwrap()),
    //     );
    // }

    let mut autopilot = Autopilot::new(bootinfo);
    autopilot.visit_kernel_fibers(bootinfo);
}
