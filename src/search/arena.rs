use std::{
    alloc,
    alloc::Layout,
    cell,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    num::NonZeroU32,
    slice,
};

pub struct Arena {
    data: *mut u8,
    orig_pointer: *mut u8,
    layout: Layout,
    next_index: cell::RefCell<NonZeroU32>,
    elem_size: usize,
    max_index: NonZeroU32,
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

pub struct SliceIndex<T> {
    data: NonZeroU32,
    length: NonZeroU32,
    phantom: PhantomData<T>,
}

impl<T> SliceIndex<T> {
    fn new(data: NonZeroU32, length: NonZeroU32) -> Self {
        Self {
            data,
            length,
            phantom: PhantomData::default(),
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
            next_index: cell::RefCell::new(NonZeroU32::new(1).unwrap()),
            elem_size,
            max_index: NonZeroU32::new(capacity + 2).unwrap(),
        })
    }

    pub fn get<'a, T>(&'a self, index: &'a Index<T>) -> &'a T {
        unsafe {
            let ptr = self.ptr_to_index(index.data) as *const T;
            &*ptr
        }
    }

    pub fn get_mut<'a, T>(&'a self, index: &'a mut Index<T>) -> &'a mut T {
        unsafe {
            let ptr = self.ptr_to_index(index.data) as *mut T;
            &mut *ptr
        }
    }

    pub fn get_slice<'a, T>(&'a self, index: &'a SliceIndex<T>) -> &'a [T] {
        unsafe {
            let ptr = self.ptr_to_index(index.data) as *const T;
            slice::from_raw_parts(ptr, index.length.get() as usize)
        }
    }

    pub fn get_slice_mut<'a, T>(&'a self, index: &'a mut SliceIndex<T>) -> &'a mut [T] {
        unsafe {
            let ptr = self.ptr_to_index(index.data) as *mut T;
            slice::from_raw_parts_mut(ptr, index.length.get() as usize)
        }
    }

    pub fn add<T>(&self, value: T) -> Index<T> {
        // Check that the arena supports this value
        assert!(self.supports_type::<T>());

        let mut raw_next_index = self.next_index.borrow_mut();

        assert!(
            raw_next_index
                .get()
                .checked_add(self.bucket_size::<T>())
                .unwrap()
                <= self.max_index.get()
        );

        let ptr = unsafe { self.ptr_to_index(*raw_next_index) as *mut MaybeUninit<T> };

        unsafe {
            (*ptr).write(value);
        }

        let old_index = Index::new(*raw_next_index);

        *raw_next_index = NonZeroU32::new(raw_next_index.get() + self.bucket_size::<T>()).unwrap();

        old_index
    }

    pub fn add_slice<T>(&self, values: &mut Vec<T>) -> SliceIndex<T> {
        assert!(self.supports_type::<T>());
        let length = values.len();
        assert_ne!(length, 0);

        let mut raw_index = self.next_index.borrow_mut();

        let raw_new_index = NonZeroU32::new(
            raw_index
                .get()
                .checked_add(self.bucket_size::<T>() * values.len() as u32)
                .unwrap(),
        )
        .unwrap();

        assert!(raw_new_index <= self.max_index);

        let mut ptr = unsafe { self.ptr_to_index(*raw_index) as *mut MaybeUninit<T> };

        for value in values.drain(..) {
            unsafe {
                (*ptr).write(value);
                ptr = ptr.add(self.bucket_size::<T>() as usize);
            }
        }

        let old_index = SliceIndex::new(*raw_index, NonZeroU32::new(length as u32).unwrap());

        *raw_index = raw_new_index;

        old_index
    }

    pub const fn supports_type<T>(&self) -> bool {
        mem::size_of::<T>() % self.elem_size == 0 && self.elem_size % mem::align_of::<T>() == 0
    }

    unsafe fn ptr_to_index(&self, raw_index: NonZeroU32) -> *const u8 {
        self.data.add(raw_index.get() as usize * self.elem_size)
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
