//! Network device transports (MCP/Netmiko/serial) are adapters, never domain dependencies.
pub trait DeviceTransport: Send + Sync { fn observe(&self, target: &str, request: &str) -> Result<String, String>; fn apply(&self, target: &str, command: &str) -> Result<String, String>; }

pub struct FnDeviceTransport<O, A> { pub observe_fn: O, pub apply_fn: A }
impl<O, A> DeviceTransport for FnDeviceTransport<O, A>
where O: Fn(&str, &str) -> Result<String, String> + Send + Sync,
      A: Fn(&str, &str) -> Result<String, String> + Send + Sync {
    fn observe(&self, target: &str, request: &str) -> Result<String, String> { (self.observe_fn)(target, request) }
    fn apply(&self, target: &str, command: &str) -> Result<String, String> { (self.apply_fn)(target, command) }
}
