use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;

use ftl_types::error::ErrorCode;
use ftl_types::nic::RouteRule;

use crate::nic::Nic;
use crate::packet::ipv4::Ipv4Addr;

struct Route<N> {
    rule: RouteRule,
    dst: Weak<Nic<N>>,
}

pub(crate) struct RouteTable<N> {
    /// Routes sorted by the prefix length, longest first.
    routes: Vec<Route<N>>,
}

impl<N> RouteTable<N> {
    pub(crate) const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    fn find_overlap(&self, rule: &RouteRule) -> Option<&Route<N>> {
        for route in &self.routes {
            if route.rule.overlaps(rule) {
                return Some(route);
            }
        }

        None
    }

    pub(crate) fn bind(&mut self, rule: &RouteRule, dst: &Arc<Nic<N>>) -> Result<(), ErrorCode> {
        if !rule.is_valid() {
            return Err(ErrorCode::InvalidArg);
        }

        self.routes.retain(|route| route.dst.strong_count() > 0);

        if self.find_overlap(rule).is_some() {
            return Err(ErrorCode::AlreadyExists);
        }

        let mut index = self.routes.len();
        for (i, route) in self.routes.iter().enumerate() {
            if route.rule.prefix_len < rule.prefix_len {
                index = i;
                break;
            }
        }

        self.routes
            .try_reserve(1)
            .map_err(|_| ErrorCode::OutOfMemory)?;
        self.routes.insert(
            index,
            Route {
                rule: *rule,
                dst: Arc::downgrade(dst),
            },
        );
        Ok(())
    }

    pub(crate) fn unbind(&mut self, rule: &RouteRule) -> Result<(), ErrorCode> {
        for i in 0..self.routes.len() {
            if self.routes[i].rule == *rule {
                self.routes.remove(i);
                return Ok(());
            }
        }

        Err(ErrorCode::NotFound)
    }

    /// Returns the destination NIC and source IP address to overwrite.
    pub(crate) fn lookup(
        &self,
        protocols: u8,
        dst_ip: Ipv4Addr,
        dst_port: u16,
    ) -> Option<(Arc<Nic<N>>, Option<Ipv4Addr>)> {
        for route in &self.routes {
            if !route.rule.matches(protocols, dst_ip.as_u32(), dst_port) {
                continue;
            }

            // Overwrite the source IP address if the route specifies one.
            let src_ip = match route.rule.src_ip {
                0 => None,
                src_ip => Some(Ipv4Addr::new(src_ip)),
            };

            let Some(dst) = route.dst.upgrade() else {
                // The NIC has been dropped, without removing this route.
                continue;
            };

            return Some((dst, src_ip));
        }

        None
    }
}
