use alloc::collections::BTreeMap;
use alloc::ffi::CString;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;

use ftl_types::environ;
use ftl_types::environ::Environ;
use ftl_types::handle::HandleId;
use ftl_types::message::Message;
use ftl_types::spec::DeviceTreeEntry;
use ftl_types::spec::FiberSpec;

use crate::boot::BootInfo;
use crate::channel::Channel;
use crate::fiber::Fiber;
use crate::fiber::Object;
use crate::lock::Mutex;

mod device_tree;

struct ResolvedDep {
    provide_name: String,
    client_channel: Channel,
}

struct DesiredFiber {
    spec: FiberSpec,
    main: fn(*const i8),
    resolved_deps: Vec<ResolvedDep>,
    our_control_ch: Channel,
}

struct Desired {
    fibers: Vec<DesiredFiber>,
    server_channels: BTreeMap<String /* provide name */, Vec<Channel>>,
    client_channels: BTreeMap<String /* provide name */, Vec<Channel>>,
}

impl Desired {
    pub fn new() -> Desired {
        Desired {
            fibers: Vec::new(),
            server_channels: BTreeMap::new(),
            client_channels: BTreeMap::new(),
        }
    }

    pub fn load_bootinfo(&mut self, bootinfo: &BootInfo) {
        self.visit_kernel_fibers(bootinfo);
    }

    fn visit_kernel_fibers(&mut self, bootinfo: &BootInfo) {
        let mut fiber_specs = BTreeMap::new();
        let mut kernel_fiber_entries = BTreeMap::new();
        for (spec, main) in bootinfo.kernel_fibers.iter() {
            let spec: FiberSpec =
                serde_json::from_str(spec).expect("failed to parse an in-kernel fiber spec");

            let fiber_name = spec.name.clone();
            fiber_specs.insert(fiber_name.clone(), spec);
            kernel_fiber_entries.insert(fiber_name, main);
        }

        for (fiber_name, spec) in fiber_specs.iter() {
            let (control_ch1, control_ch2) = Channel::new().unwrap();
            let mut resolved_deps = Vec::new();
            resolved_deps.push(ResolvedDep {
                provide_name: "autopilot".to_string(),
                client_channel: control_ch1,
            });

            for dep in &spec.deps {
                if dep == "autopilot" {
                    continue;
                }

                let Some(dep_fiber) = fiber_specs
                    .values()
                    .find(|spec| spec.provides.iter().any(|provide| provide == dep))
                else {
                    panic!(
                        "fiber {} depends on {}, but no such fiber exists",
                        fiber_name, dep
                    );
                };

                let (client_ch, server_ch) = Channel::new().unwrap();
                self.server_channels
                    .entry(dep_fiber.name.clone())
                    .or_insert(Vec::new())
                    .push(server_ch);

                resolved_deps.push(ResolvedDep {
                    provide_name: dep.to_string(),
                    client_channel: client_ch,
                });
            }

            self.fibers.push(DesiredFiber {
                spec: fiber_specs[fiber_name].clone(),
                main: *kernel_fiber_entries[fiber_name],
                resolved_deps,
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

    fn find_devices_for_fiber(&self, patterns: &[DeviceTreeEntry]) -> Vec<environ::Device> {
        let mut devices = Vec::new();
        for pattern in patterns {
            for device in self.devices.iter() {
                if device.compatible == pattern.compatible {
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

                    devices.push(ftl_types::environ::Device {
                        name: device.name.to_string(),
                        compatible: device.compatible.to_string(),
                        reg: device.reg,
                        interrupts,
                    });
                }
            }
        }

        devices
    }

    pub fn apply_desired(&mut self, mut desired: Desired) {
        debug_assert!(
            self.fibers.is_empty(),
            "TODO: diffing states are not implemented"
        );

        for DesiredFiber {
            spec,
            main,
            our_control_ch,
            resolved_deps,
        } in desired.fibers
        {
            let mut devices = if let Some(entries) = spec.device_tree.as_ref() {
                let devices = self.find_devices_for_fiber(entries);
                if devices.is_empty() {
                    panic!("no devices found for fiber {}", spec.name);
                }

                Some(devices)
            } else {
                None
            };

            let mut next_handle_id = 1;
            let mut alloc_handle_id = || {
                let handle_id = HandleId::from_isize(next_handle_id);
                next_handle_id += 1;
                handle_id
            };

            let mut fiber = Fiber::new();
            let mut deps = BTreeMap::new();
            for ResolvedDep {
                provide_name,
                client_channel,
            } in resolved_deps
            {
                let handle_id = alloc_handle_id();
                fiber.insert_handle(handle_id, Object::Channel(client_channel));
                deps.insert(provide_name.to_string(), handle_id);
            }

            if let Some(server_channels) = desired.server_channels.remove(&spec.name) {
                for ch in server_channels {
                    let handle_id = alloc_handle_id();
                    fiber.insert_handle(handle_id, Object::Channel(ch));

                    // Non-blocking send to the fiber to tell it about the
                    // pre-existing client channels.
                    our_control_ch
                        .send(Message::NewClient { ch: handle_id })
                        .unwrap();
                }
            }

            let environ = Environ { deps, devices };
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
