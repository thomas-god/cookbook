use std::time::SystemTime;

use cookbook_domain::clock::Clock as DomainClock;

use crate::auth::ports::Clock as AuthClock;

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SystemClock {
    pub fn new() -> Self {
        Self
    }
}

impl AuthClock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

impl DomainClock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}
