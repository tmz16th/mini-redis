use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::Path,
};

/// The database: an in-memory map plus an append-only log on disk.
pub struct Store {
    data: HashMap<String, String>,
    /// `None` for the in-memory stores used in tests.
    log: Option<BufWriter<File>>,
}

impl Store {
    /// Open (or create) a log file and rebuild the map by replaying it.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .append(true)
            .open(path)?;

        let mut store = Self {
            data: HashMap::new(),
            log: None,
        };
        store.replay(&file)?;
        store.log = Some(BufWriter::new(file));
        Ok(store)
    }

    /// A store with no persistence, for tests and benchmarks.
    pub fn in_memory() -> Self {
        Self {
            data: HashMap::new(),
            log: None,
        }
    }

    /// Rebuild the map from the log.
    ///
    /// Applies records straight to `data` rather than going through `set`,
    /// which would append them to the log a second time.
    fn replay(&mut self, file: &File) -> io::Result<()> {
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }

            let args: Vec<&str> = line.trim().split_whitespace().collect();
            match args.as_slice() {
                ["SET", key, value] => self.apply_set((*key).into(), (*value).into()),
                ["DEL", key] => {
                    self.apply_remove(key);
                }
                _ => eprintln!("warning: skipping malformed log record: {:?}", line.trim()),
            }
        }

        Ok(())
    }

    /// Append a record and force it to disk before returning.
    fn append(&mut self, record: String) -> io::Result<()> {
        if let Some(log) = self.log.as_mut() {
            log.write_all(record.as_bytes())?;
            log.flush()?;
            // flush() only reaches the OS buffer; sync_data() reaches the disk.
            log.get_ref().sync_data()?;
        }
        Ok(())
    }

    fn apply_set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    fn apply_remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Log first, then update memory: if the disk write fails, memory is unchanged.
    pub fn set(&mut self, key: String, value: String) -> io::Result<()> {
        self.append(format!("SET {key} {value}\n"))?;
        self.apply_set(key, value);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    pub fn remove(&mut self, key: &str) -> io::Result<Option<String>> {
        self.append(format!("DEL {key}\n"))?;
        Ok(self.apply_remove(key))
    }
}

#[cfg(test)]
mod tests {
    use super::Store;
    use std::{fs, path::PathBuf};

    fn log_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mini-redis-{name}-{}.wal", std::process::id()))
    }

    #[test]
    fn set_then_get() {
        let mut store = Store::in_memory();
        store.set("a".into(), "1".into()).unwrap();
        assert_eq!(store.get("a"), Some("1"));
    }

    #[test]
    fn missing_key_returns_none() {
        let store = Store::in_memory();
        assert_eq!(store.get("nope"), None);
    }

    #[test]
    fn set_overwrites() {
        let mut store = Store::in_memory();
        store.set("a".into(), "1".into()).unwrap();
        store.set("a".into(), "2".into()).unwrap();
        assert_eq!(store.get("a"), Some("2"));
    }

    #[test]
    fn data_survives_restart() {
        let path = log_path("restart");
        let _ = fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            store.set("name".into(), "Ted".into()).unwrap();
            store.set("count".into(), "42".into()).unwrap();
        }

        let store = Store::open(&path).unwrap();
        assert_eq!(store.get("name"), Some("Ted"));
        assert_eq!(store.get("count"), Some("42"));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn delete_survives_restart() {
        let path = log_path("delete");
        let _ = fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            store.set("name".into(), "Ted".into()).unwrap();
            store.remove("name").unwrap().unwrap();
        }

        let store = Store::open(&path).unwrap();
        assert_eq!(store.get("name"), None);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn latest_value_wins_across_restarts() {
        let path = log_path("overwrite");
        let _ = fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            store.set("a".into(), "1".into()).unwrap();
        }
        {
            let mut store = Store::open(&path).unwrap();
            store.set("a".into(), "2".into()).unwrap();
        }

        let store = Store::open(&path).unwrap();
        assert_eq!(store.get("a"), Some("2"));
        fs::remove_file(&path).unwrap();
    }

    /// Run with: cargo test --release -- --ignored --nocapture
    #[test]
    #[ignore]
    fn memory_throughput() {
        use std::time::Instant;

        const OPS: u32 = 1_000_000;
        let mut store = Store::in_memory();

        let start = Instant::now();
        for i in 0..OPS {
            store.set(format!("key{i}"), format!("value{i}")).unwrap();
        }
        let writes = start.elapsed();

        let start = Instant::now();
        for i in 0..OPS {
            assert!(store.get(&format!("key{i}")).is_some());
        }
        let reads = start.elapsed();

        println!("memory SET: {:.0} ops/sec", OPS as f64 / writes.as_secs_f64());
        println!("memory GET: {:.0} ops/sec", OPS as f64 / reads.as_secs_f64());
    }

    #[test]
    #[ignore]
    fn wal_throughput() {
        use std::time::Instant;

        const OPS: u32 = 200;
        // Use the real filesystem, not /tmp: tmpfs makes fsync free and
        // would give a misleadingly good durability number.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bench.wal");
        let _ = fs::remove_file(&path);
        let mut store = Store::open(&path).unwrap();

        let start = Instant::now();
        for i in 0..OPS {
            store.set(format!("key{i}"), format!("value{i}")).unwrap();
        }
        let elapsed = start.elapsed();

        println!(
            "WAL SET (fsync every write): {:.0} ops/sec",
            OPS as f64 / elapsed.as_secs_f64()
        );
        drop(store);
        fs::remove_file(&path).unwrap();
    }
}
