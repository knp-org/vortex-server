//! Simple in-memory cache for frequently accessed data
//!
//! Provides thread-safe caching with TTL expiration.
//! Note: This module is ready for future use.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// A cache entry with value and expiration time
struct CacheEntry<V> {
    value: V,
    expires_at: Instant,
}

/// Simple thread-safe cache with TTL
pub struct Cache<K, V> {
    data: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    default_ttl: Duration,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Cache<K, V> {
    /// Create a new cache with default TTL
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(ttl_seconds),
        }
    }

    /// Get a value from the cache
    pub fn get(&self, key: &K) -> Option<V> {
        let data = self.data.read().ok()?;
        let entry = data.get(key)?;
        
        if entry.expires_at > Instant::now() {
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Set a value in the cache with default TTL
    pub fn set(&self, key: K, value: V) {
        self.set_with_ttl(key, value, self.default_ttl);
    }

    /// Set a value in the cache with custom TTL
    pub fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        if let Ok(mut data) = self.data.write() {
            data.insert(key, CacheEntry {
                value,
                expires_at: Instant::now() + ttl,
            });
        }
    }

    /// Remove a value from the cache
    pub fn remove(&self, key: &K) {
        if let Ok(mut data) = self.data.write() {
            data.remove(key);
        }
    }

    /// Clear all expired entries
    pub fn cleanup(&self) {
        if let Ok(mut data) = self.data.write() {
            let now = Instant::now();
            data.retain(|_, entry| entry.expires_at > now);
        }
    }

    /// Get or set a value using a factory function
    pub fn get_or_insert<F>(&self, key: K, factory: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(value) = self.get(&key) {
            return value;
        }

        let value = factory();
        self.set(key, value.clone());
        value
    }
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            default_ttl: self.default_ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cache_basic() {
        let cache: Cache<String, i32> = Cache::new(60);
        cache.set("key".to_string(), 42);
        assert_eq!(cache.get(&"key".to_string()), Some(42));
    }

    #[test]
    fn test_cache_expiry() {
        let cache: Cache<String, i32> = Cache::new(1);
        cache.set_with_ttl("key".to_string(), 42, Duration::from_millis(50));
        assert_eq!(cache.get(&"key".to_string()), Some(42));
        thread::sleep(Duration::from_millis(100));
        assert_eq!(cache.get(&"key".to_string()), None);
    }
}
