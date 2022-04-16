use std::{
    alloc,
    alloc::Layout,
    any, fmt,
    marker::PhantomData,
    mem,
    num::NonZeroU32,
    slice,
    sync::atomic::{AtomicU32, Ordering},
};

pub struct Arena {
    data: *mut u8,
    orig_pointer: *mut u8,
    layout: Layout,
    next_index: AtomicU32,
    elem_size: usize,
    max_index: u32,
}

impl fmt::Debug for Arena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Arena")
            .field("next_index", &self.next_index)
            .field("elem_size", &self.elem_size)
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
            phantom: PhantomData::default(),
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
            phantom: PhantomData::default(),
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

impl Arena {
    pub fn new(capacity: u32, elem_size: usize) -> Option<Self> {
        if elem_size == 0 || capacity == 0 || capacity >= u32::MAX - 1 {
            return None;
        }
        let raw_alignment = raw_alignment(elem_size);

        let layout =
            Layout::from_size_align((capacity as usize + 2) * elem_size, raw_alignment).ok()?;

        let (data, orig_pointer) = unsafe {
            let ptr = alloc::alloc(layout);

            // Make sure the pointer is correctly aligned
            if (ptr as usize) % elem_size == 0 {
                (ptr, ptr)
            } else {
                (ptr.add(elem_size - (ptr as usize) % elem_size), ptr)
            }
        };

        Some(Self {
            data,
            orig_pointer,
            layout,
            next_index: AtomicU32::new(1),
            elem_size,
            max_index: capacity + 2,
        })
    }

    pub fn get<'a, T>(&'a self, index: &'a Index<T>) -> &'a T {
        unsafe {
            let ptr = self.ptr_to_index(index.data.get()) as *const T;
            &*ptr
        }
    }

    pub fn get_mut<'a, T>(&'a self, index: &'a mut Index<T>) -> &'a mut T {
        unsafe {
            let ptr = self.ptr_to_index(index.data.get()) as *mut T;
            &mut *ptr
        }
    }

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
            self.elem_size
        );

        let index = self.get_index_for_element(self.bucket_size::<T>())?;

        let ptr = unsafe { self.ptr_to_index(index) as *mut T };

        unsafe {
            *ptr = value;
        }

        Some(Index::new(NonZeroU32::new(index).unwrap()))
    }

    pub fn add_slice<T>(&self, values: &mut Vec<T>) -> Option<SliceIndex<T>> {
        assert!(self.supports_type::<T>());
        let length = values.len();
        assert_ne!(length, 0);

        let index = self.get_index_for_element(self.bucket_size::<T>() * values.len() as u32)?;

        let mut ptr = unsafe { self.ptr_to_index(index) as *mut T };

        for value in values.drain(..) {
            unsafe {
                *ptr = value;
                ptr = ptr.add(1);
            }
        }

        Some(SliceIndex::new(
            NonZeroU32::new(index).unwrap(),
            length as u32,
        ))
    }

    /// Gets an appropriate index for the new element, if there is space available
    fn get_index_for_element(&self, size: u32) -> Option<u32> {
        self.next_index
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |index| {
                index
                    .checked_add(size)
                    .filter(|next_index| *next_index <= self.max_index)
            })
            .ok()
    }

    pub const fn supports_type<T>(&self) -> bool {
        mem::size_of::<T>() % self.elem_size == 0 && self.elem_size % mem::align_of::<T>() == 0
    }

    unsafe fn ptr_to_index(&self, raw_index: u32) -> *const u8 {
        self.data.add(raw_index as usize * self.elem_size)
    }

    const fn bucket_size<T>(&self) -> u32 {
        (mem::size_of::<T>() / self.elem_size) as u32
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.orig_pointer, self.layout);
        }
    }
}
