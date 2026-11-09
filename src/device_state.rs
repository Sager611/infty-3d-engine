//! Shared terminal device state; callbacks mark loss, adapters check before GPU work.
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct DeviceState(Arc<AtomicBool>);

impl DeviceState {
    pub fn lose(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn check(&self) -> Result<(), &'static str> {
        if self.0.load(Ordering::Relaxed) {
            Err("GPU device lost; reload to recover")
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceState;

    #[test]
    fn healthy_device_is_ready() {
        assert_eq!(DeviceState::default().check(), Ok(()));
    }

    #[test]
    fn loss_is_shared_terminal_and_idempotent() {
        let state = DeviceState::default();
        let callback = state.clone();
        callback.lose();
        callback.lose();
        assert_eq!(state.check(), Err("GPU device lost; reload to recover"));
        assert_eq!(callback.check(), state.check());
    }
}
