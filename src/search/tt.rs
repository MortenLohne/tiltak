pub struct TT {
    buckets: Vec<Bucket>,
    generation: u8,
}

impl TT {
    pub fn new(size: usize) -> Self {
        TT {
            buckets: vec![Bucket::default(); size],
            generation: 1,
        }
    }

    pub fn increment_generation(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn used_entries(&self) -> usize {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.entries.iter())
            .count()
    }

    pub fn get(&self, hash: u64) -> Option<f32> {
        let index = hash as usize % self.buckets.len();
        let bucket = &self.buckets[index];
        let upper_bits = (hash >> 32) as u32;
        for entry in bucket.entries.iter().flatten() {
            if entry.hash_key_upper_bits == upper_bits {
                return Some(entry.value);
            }
        }
        None
    }

    pub fn insert(&mut self, hash: u64, value: f32, visits: u32) {
        let index = hash as usize % self.buckets.len();
        let bucket = &mut self.buckets[index];
        let upper_bits = (hash >> 32) as u32;

        let log2_visits = visits.ilog2() as u8;

        bucket.insert(upper_bits, value, self.generation, log2_visits);
    }
}

#[derive(Debug, Clone, Default)]
struct Bucket {
    entries: [Option<Entry>; 4],
}

impl Bucket {
    fn insert(&mut self, hash_key_upper_bits: u32, value: f32, generation: u8, log2_visits: u8) {
        let new_entry = Entry::new(hash_key_upper_bits, value, generation, log2_visits);

        let lowest_pri_entry = self
            .entries
            .iter_mut()
            .min_by_key(|entry| entry.as_ref().map_or(0, |entry| entry.insertion_value()))
            .unwrap();
        if lowest_pri_entry
            .as_mut()
            .is_none_or(|entry| entry.insertion_value() <= new_entry.insertion_value())
        {
            *lowest_pri_entry = Some(new_entry);
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    hash_key_upper_bits: u32,
    value: f32,
    generation: u8,
    log2_visits: u8,
}

impl Entry {
    fn new(hash_key_upper_bits: u32, value: f32, generation: u8, log2_visits: u8) -> Self {
        Entry {
            hash_key_upper_bits,
            value,
            generation,
            log2_visits,
        }
    }
    fn insertion_value(&self) -> u16 {
        self.generation as u16 + self.log2_visits as u16
    }
}

#[test]
fn insert_4_values_test() {
    let mut tt = TT::new(1);
    for i in 0..4 {
        let hash = i << 32 + i;
        tt.insert(hash, i as f32, 1);
    }
    println!("Used entries: {}", tt.used_entries());

    for i in 0..4 {
        let hash = i << 32 + i;
        assert_eq!(tt.get(hash), Some(i as f32));
    }
}

#[test]
fn overwrite_values_test() {
    let mut tt = TT::new(1);
    for i in 0..4 {
        let hash = i << 32 + i;
        tt.insert(hash, i as f32, 1);
    }

    tt.generation = 10;
    // Higher generation entries should be prioritized
    for i in 0..4 {
        let hash = i << 32 + i;
        tt.insert(hash, i as f32 + 10.0, 1);
    }
    for i in 0..4 {
        let hash = i << 32 + i;
        assert_eq!(tt.get(hash), Some(i as f32 + 10.0));
    }

    tt.generation = 9;

    // Slightly lower generation entries with very high visits counts should be prioritized even more
    for i in 0..4 {
        let hash = i << 32 + i;
        tt.insert(hash, i as f32 + 100.0, 1_000_000);
    }
    for i in 0..4 {
        let hash = i << 32 + i;
        assert_eq!(tt.get(hash), Some(i as f32 + 100.0));
    }
}
