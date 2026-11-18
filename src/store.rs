use std::collections::HashMap;

/// The actual database. For now it only lives in memory.
///
/// Keeping this separate from main.rs means we can later swap the REPL for a
/// TCP server without touching a single line of storage logic.
#[derive(Debug, Default)]
pub struct Store {
    data: HashMap<String, String>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or overwrite a key.
    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    /// Look up a key. Returns a borrow of the value so we don't copy it.
    /// `None` means the key isn't there.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    /// Remove a key, returning the old value if there was one.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::Store;

    #[test]
    fn set_then_get() {
        let mut store = Store::new();
        store.set("a".into(), "1".into());
        assert_eq!(store.get("a"), Some("1"));
    }

    #[test]
    fn missing_key_returns_none() {
        let store = Store::new();
        assert_eq!(store.get("nope"), None);
    }

    #[test]
    fn set_overwrites() {
        let mut store = Store::new();
        store.set("a".into(), "1".into());
        store.set("a".into(), "2".into());
        assert_eq!(store.get("a"), Some("2"));
    }

    /// Run with: cargo test --release -- --ignored --nocapture
    /// Ignored by default so normal test runs stay instant.
    #[test]
    #[ignore]
    fn rough_throughput() {
        use std::time::Instant;

        const OPS: u32 = 1_000_000;
        let mut store = Store::new();

        let start = Instant::now();
        for i in 0..OPS {
            store.set(format!("key{i}"), format!("value{i}"));
        }
        let writes = start.elapsed();

        let start = Instant::now();
        for i in 0..OPS {
            assert!(store.get(&format!("key{i}")).is_some());
        }
        let reads = start.elapsed();

        println!("SET: {:.0} ops/sec", OPS as f64 / writes.as_secs_f64());
        println!("GET: {:.0} ops/sec", OPS as f64 / reads.as_secs_f64());
    }
}
