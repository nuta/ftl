use alloc::collections::BTreeMap;
use alloc::ffi::CString;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_types::environ::Environ;
use ftl_types::handle::HandleId;
use ftl_types::spec::DeviceTreeEntry;
use ftl_types::spec::FiberSpec;

use crate::boot::BootInfo;
use crate::channel::Channel;
use crate::fiber::Fiber;
use crate::fiber::Object;
use crate::lock::Mutex;

mod device_tree;

struct DesiredFiber {
    spec: FiberSpec,
    main: fn(*const i8),
    their_control_ch: Channel,
    our_control_ch: Channel,
}

struct Desired {
    fibers: Vec<DesiredFiber>,
}

impl Desired {
    pub fn new() -> Desired {
        Desired { fibers: Vec::new() }
    }

    pub fn load_bootinfo(&mut self, bootinfo: &BootInfo) {
        self.visit_kernel_fibers(bootinfo);
    }

    fn visit_kernel_fibers(&mut self, bootinfo: &BootInfo) {
        for (spec, main) in bootinfo.kernel_fibers.iter() {
            let spec: FiberSpec =
                serde_json::from_str(spec).expect("failed to parse an in-kernel fiber spec");
            let (control_ch1, control_ch2) = Channel::new().unwrap();

            self.fibers.push(DesiredFiber {
                spec,
                main: *main,
                their_control_ch: control_ch1,
                our_control_ch: control_ch2,
            });
        }
    }
}

struct ActualFiber {
    fiber: Arc<Mutex<Fiber>>,
    control_ch: Channel,
}

struct Actual {
    fibers: Vec<ActualFiber>,
    devices: Vec<device_tree::Device>,
}

impl Actual {
    pub fn new() -> Actual {
        Actual {
            fibers: Vec::new(),
            devices: Vec::new(),
        }
    }

    pub fn set_devices(&mut self, devices: Vec<device_tree::Device>) {
        debug_assert!(self.devices.is_empty());
        debug_assert!(self.fibers.is_empty());

        self.devices = devices;
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

    pub fn apply_desired(&mut self, desired: Desired) {
        debug_assert!(
            self.fibers.is_empty(),
            "TODO: diffing states are not implemented"
        );

        for DesiredFiber {
            spec,
            main,
            their_control_ch,
            our_control_ch,
        } in desired.fibers
        {
            let mut device = None;
            if let Some(entries) = spec.device_tree.as_ref() {
                match self.find_device_for_fiber(entries) {
                    Some(dev) => {
                        let interrupts = match &dev.interrupts {
                            Some(interrupts) => {
                                let mut vec = Vec::new();
                                for interrupt in interrupts {
                                    vec.push(*interrupt);
                                }
                                Some(vec)
                            }
                            None => None,
                        };

                        device = Some(ftl_types::environ::Device {
                            name: dev.name.to_string(),
                            compatible: dev.compatible.to_string(),
                            reg: dev.reg,
                            interrupts,
                        });
                    }
                    None => {
                        panic!("device not found for fiber {:?}", spec.name);
                    }
                };
            }

            let mut next_handle_id = 1;
            let mut alloc_handle_id = || {
                let handle_id = HandleId::from_isize(next_handle_id);
                next_handle_id += 1;
                handle_id
            };

            let mut fiber = Fiber::new();
            let mut deps = BTreeMap::new();
            let mut autopilot_ch = Some(their_control_ch);
            for dep_name in spec.deps.iter() {
                let handle_id = alloc_handle_id();
                let object = match dep_name.as_str() {
                    "autopilot" => Object::Channel(autopilot_ch.take().unwrap()),
                    _ => {
                        todo!();
                    }
                };

                fiber.insert_handle(handle_id, object);
                deps.insert(dep_name.to_string(), handle_id);
            }

            let environ = Environ { deps, device };
            let environ_json = serde_json::to_string(&environ).unwrap();
            let environ_cstr = CString::new(environ_json).unwrap();

            let fiber = fiber.spawn_in_kernel(move || {
                main(environ_cstr.as_ptr());
            });

            self.fibers.push(ActualFiber {
                fiber,
                control_ch: our_control_ch,
            });
        }
    }
}

struct Autopilot {
    actual: Actual,
}

impl Autopilot {
    pub fn test(bootinfo: &BootInfo, devices: Vec<device_tree::Device>) -> Autopilot {
        let mut desired = Desired::new();
        desired.load_bootinfo(bootinfo);

        let mut actual = Actual::new();
        actual.set_devices(devices);
        actual.apply_desired(desired);

        Autopilot { actual }
    }
}

pub fn start(bootinfo: &BootInfo) {
    let devices = device_tree::walk_device_nodes(bootinfo.dtb_addr);
    println!("device tree: found {} devices", devices.len());
    for device in &devices {
        println!(
            "device tree: {} (compatible \"{}\")",
            device.name, device.compatible
        );
    }

    Autopilot::test(bootinfo, devices);
}
