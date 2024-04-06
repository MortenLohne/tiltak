use std::{
    alloc::{self, Layout},
    any,
    error::Error,
    fmt::{self, Display},
    marker::PhantomData,
    mem,
    num::NonZeroU32,
    slice,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

pub struct Arena<const S: usize = 16> {
    data: *mut u8,
    orig_pointer: *mut u8,
    layout: Layout,
    next_index: AtomicU32,
    max_index: u32,
    pub stats: ArenaStats,
}

#[derive(Debug, Default)]
pub struct ArenaStats {
    pub bytes_allocated: AtomicUsize,
    pub bytes_structs: AtomicUsize,
    pub bytes_slices: AtomicUsize,
    pub padding_bytes: AtomicUsize,
}

impl<const S: usize> fmt::Debug for Arena<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Arena")
            .field("next_index", &self.next_index)
            .field("slot_size", &S)
            .field("max_index", &self.max_index)
            .finish()
    }
}

#[derive(PartialEq, Debug)]
pub struct Index<T> {
    data: NonZeroU32,
    phantom: PhantomData<T>,
}

impl<T> Index<T> {
    fn new(data: NonZeroU32) -> Self {
        Self {
            data,
            phantom: PhantomData,
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct SliceIndex<T> {
    data: NonZeroU32,
    length: u32,
    phantom: PhantomData<T>,
}

impl<T> SliceIndex<T> {
    fn new(data: NonZeroU32, length: u32) -> Self {
        Self {
            data,
            length,
            phantom: PhantomData,
        }
    }
}

impl<T> Default for SliceIndex<T> {
    fn default() -> Self {
        Self {
            data: NonZeroU32::new(1).unwrap(),
            length: 0,
            phantom: Default::default(),
        }
    }
}

const fn raw_alignment(mut alignment: usize) -> usize {
    let mut raw_alignment = 1;
    while alignment % 2 == 0 {
        raw_alignment *= 2;
        alignment /= 2;
    }
    raw_alignment
}

#[derive(Debug)]
pub enum ArenaError {
    AllocationFailed(usize),
    InvalidSettings,
}

impl Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArenaError::AllocationFailed(num_bytes) => {
                write!(f, "Failed to allocate {} bytes for arena", num_bytes)
            }
            ArenaError::InvalidSettings => write!(f, "Invalid settings for arena"),
        }
    }
}

impl Error for ArenaError {}

impl<const S: usize> Arena<S> {
    pub fn new(num_slots: u32) -> Result<Self, ArenaError> {
        if S == 0 || num_slots == 0 || num_slots >= u32::MAX - 1 {
            return Err(ArenaError::InvalidSettings);
        }
        let raw_alignment = raw_alignment(S);
        let size = (num_slots as usize + 2) * S;

        let layout = Layout::from_size_align(size, raw_alignment)
            .map_err(|_| ArenaError::InvalidSettings)?;

        let (data, orig_pointer) = unsafe {
            let ptr = alloc::alloc(layout);

            if ptr.is_null() {
                return Err(ArenaError::AllocationFailed(size));
            }

            // Make sure the pointer is correctly aligned
            if (ptr as usize) % S == 0 {
                (ptr, ptr)
            } else {
                (ptr.add(S - (ptr as usize) % S), ptr)
            }
        };

        Ok(Self {
            data,
            orig_pointer,
            layout,
            next_index: AtomicU32::new(1),
            max_index: num_slots + 1,
            stats: ArenaStats::default(),
        })
    }

    /// Get a reference to an element in the arena
    /// # Safety
    /// This function is actually unsafe, if the index is from a different arena
    pub fn get<'a, T>(&'a self, index: &'a Index<T>) -> &'a T {
        unsafe {
            let ptr = self.ptr_to_index(index.data.get()) as *const T;
            &*ptr
        }
    }

    /// Get a mutable reference to an element in the arena
    /// # Safety
    /// This function is actually unsafe, if the index is from a different arena
    pub fn get_mut<'a, T>(&'a self, index: &'a mut Index<T>) -> &'a mut T {
        unsafe {
            let ptr = self.ptr_to_index(index.data.get()) as *mut T;
            &mut *ptr
        }
    }

    /// Get a slice from the arena
    /// # Safety
    /// This function is actually unsafe, if the index is from a different arena
    pub fn get_slice<'a, T>(&'a self, index: &'a SliceIndex<T>) -> &'a [T] {
        if index.length == 0 {
            Default::default()
        } else {
            unsafe {
                let ptr = self.ptr_to_index(index.data.get()) as *const T;
                slice::from_raw_parts(ptr, index.length as usize)
            }
        }
    }

    /// Get a mutable slice from the arena
    /// # Safety
    /// This function is actually unsafe, if the index is from a different arena
    pub fn get_slice_mut<'a, T>(&'a self, index: &'a mut SliceIndex<T>) -> &'a mut [T] {
        if index.length == 0 {
            Default::default()
        } else {
            unsafe {
                let ptr = self.ptr_to_index(index.data.get()) as *mut T;
                slice::from_raw_parts_mut(ptr, index.length as usize)
            }
        }
    }

    pub fn add<T>(&self, value: T) -> Option<Index<T>> {
        // Check that the arena supports this value
        assert!(
            self.supports_type::<T>(),
            "cannot store {} of size {} and alignment {} in arena with size {}",
            any::type_name::<T>(),
            mem::size_of::<T>(),
            mem::align_of::<T>(),
            S
        );

        let index = self.get_index_for_element(Self::num_slots_required::<T>())?;

        let ptr = unsafe { self.ptr_to_index(index) as *mut T };

        unsafe {
            *ptr = value;
        }

        let bytes_required = Self::num_slots_required::<T>() as usize * S;
        self.stats
            .bytes_allocated
            .fetch_add(bytes_required, Ordering::Relaxed);
        self.stats
            .padding_bytes
            .fetch_add(bytes_required - mem::size_of::<T>(), Ordering::Relaxed);
        self.stats
            .bytes_structs
            .fetch_add(bytes_required, Ordering::Relaxed);

        Some(Index::new(NonZeroU32::new(index).unwrap()))
    }

    pub fn add_slice<T, I: ExactSizeIterator<Item = T>>(
        &self,
        mut source: I,
    ) -> Option<SliceIndex<T>> {
        assert!(self.supports_type::<T>());

        let length = source.len();

        if length == 0 {
            return Some(SliceIndex::default());
        }
        if length > u32::MAX as usize {
            return None;
        }

        let index = self.get_index_for_element(Self::num_slots_required::<T>() * length as u32)?;

        let mut ptr = unsafe { self.ptr_to_index(index) as *mut T };

        for _ in 0..length {
            unsafe {
                *ptr = source.next().expect("Iterator yielded too few items");
                ptr = ptr.add(1);
            }
        }

        assert!(source.next().is_none());

        let bytes_required = Self::num_slots_required::<T>() as usize * length * S;
        self.stats
            .bytes_allocated
            .fetch_add(bytes_required, Ordering::Relaxed);
        self.stats.padding_bytes.fetch_add(
            bytes_required - mem::size_of::<T>() * length,
            Ordering::Relaxed,
        );
        self.stats
            .bytes_slices
            .fetch_add(bytes_required, Ordering::Relaxed);

        Some(SliceIndex::new(
            NonZeroU32::new(index).unwrap(),
            length as u32,
        ))
    }

    /// Gets an appropriate index for the new element, if there is space available
    fn get_index_for_element(&self, slots: u32) -> Option<u32> {
        self.next_index
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |index| {
                index
                    .checked_add(slots)
                    .filter(|next_index| *next_index <= self.max_index)
            })
            .ok()
    }

    pub const fn supports_type<T>(&self) -> bool {
        S % mem::align_of::<T>() == 0
    }

    pub fn slots_used(&self) -> u32 {
        self.next_index.load(Ordering::SeqCst) - 1
    }

    unsafe fn ptr_to_index(&self, raw_index: u32) -> *const u8 {
        self.data.add(raw_index as usize * S)
    }

    const fn num_slots_required<T>() -> u32 {
        let q = (mem::size_of::<T>() / S) as u32;
        let rem = (mem::size_of::<T>() % S) as u32;
        if rem == 0 {
            q
        } else {
            q + 1
        }
    }
}

impl<const S: usize> Drop for Arena<S> {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.orig_pointer, self.layout);
        }
    }
}
