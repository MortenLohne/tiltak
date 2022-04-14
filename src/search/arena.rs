use std::{
    alloc,
    alloc::Layout,
    cell,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    num::NonZeroU32,
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

            // Make sure the data starts at an address divisible by `elem_size`
            (ptr.add(elem_size - (ptr as usize) % elem_size), ptr)
        };

        Some(Self {
            data,
            orig_pointer,
            layout,
            next_index: cell::RefCell::new(NonZeroU32::new(1).unwrap()),
            elem_size,
            max_index: NonZeroU32::new(capacity + 1).unwrap(),
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

    pub fn add<T>(&self, value: T) -> Index<T> {
        // Check that the arena supports this value
        assert!(self.supports_type::<T>());

        let mut raw_next_index = self.next_index.borrow_mut();

        assert!(*raw_next_index <= self.max_index);

        let ptr = unsafe { self.ptr_to_index(*raw_next_index) as *mut MaybeUninit<T> };

        unsafe {
            (*ptr).write(value);
        }

        let old_index = Index::new(*raw_next_index);

        *raw_next_index = NonZeroU32::new(raw_next_index.get() + 1).unwrap();

        old_index
    }

    pub const fn supports_type<T>(&self) -> bool {
        mem::size_of::<T>() % self.elem_size == 0 && self.elem_size % mem::align_of::<T>() == 0
    }

    unsafe fn ptr_to_index(&self, raw_index: NonZeroU32) -> *const u8 {
        self.data.add(raw_index.get() as usize * self.elem_size)
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.orig_pointer, self.layout);
        }
    }
}
