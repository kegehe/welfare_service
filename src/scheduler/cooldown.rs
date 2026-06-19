use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

/// Tracks short-lived upstream rate-limit cooldowns for pool keys.
pub struct RateLimitCooldown {
    entries: RwLock<HashMap<i64, Instant>>,
    default_duration: Duration,
}

impl RateLimitCooldown {
    pub fn new(default_secs: u64) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            default_duration: Duration::from_secs(default_secs),
        }
    }

    pub fn mark_limited(&self, key_id: i64) {
        self.mark_limited_for(key_id, self.default_duration);
    }

    pub fn mark_limited_for(&self, key_id: i64, duration: Duration) {
        let until = Instant::now() + duration;
        self.entries.write().insert(key_id, until);
    }

    pub fn is_allowed(&self, key_id: i64) -> bool {
        let now = Instant::now();
        {
            let entries = self.entries.read();
            match entries.get(&key_id) {
                Some(until) if *until > now => return false,
                Some(_) => {}
                None => return true,
            }
        }

        self.entries.write().remove(&key_id);
        true
    }

    pub fn unregister(&self, key_id: i64) {
        self.entries.write().remove(&key_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_blocked_during_cooldown() {
        let cooldown = RateLimitCooldown::new(30);

        cooldown.mark_limited_for(1, Duration::from_secs(30));

        assert!(!cooldown.is_allowed(1));
        assert!(cooldown.is_allowed(2));
    }

    #[test]
    fn expired_cooldown_is_removed() {
        let cooldown = RateLimitCooldown::new(30);

        cooldown.mark_limited_for(1, Duration::from_secs(0));

        assert!(cooldown.is_allowed(1));
        assert!(cooldown.is_allowed(1));
    }
}
