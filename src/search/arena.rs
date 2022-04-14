use std::{cell, marker::PhantomData, mem, num::NonZeroU32};

pub struct Arena {
    data: Box<[cell::RefCell<[u8; 32]>]>,
    next_index: cell::Cell<NonZeroU32>,
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

impl Arena {
    pub fn new(capacity: usize) -> Self {
        let mut data_vec = Vec::with_capacity(capacity);
        while data_vec.len() < data_vec.capacity() {
            data_vec.push(cell::RefCell::new([0; 32]));
        }
        Self {
            data: data_vec.into_boxed_slice(),
            next_index: cell::Cell::new(NonZeroU32::new(1).unwrap()),
        }
    }

    pub fn get<T>(&self, index: &Index<T>) -> cell::Ref<T> {
        cell::Ref::map(
            self.data[index.data.get() as usize].borrow(),
            |arr| unsafe { mem::transmute(arr) },
        )
    }

    pub fn get_mut<T>(&self, index: &mut Index<T>) -> cell::RefMut<T> {
        cell::RefMut::map(
            self.data[index.data.get() as usize].borrow_mut(),
            |arr| unsafe { mem::transmute(arr) },
        )
    }

    pub fn add<T>(&self, value: T) -> Index<T> {
        // Check that the arena supports this value
        assert_eq!(mem::size_of::<T>(), mem::size_of::<[u8; 32]>());

        let old_index_raw = self
            .next_index
            .replace(NonZeroU32::new(self.next_index.get().get() + 1).unwrap());
        let mut old_index = Index::new(old_index_raw);
        *self.get_mut(&mut old_index) = value;
        old_index
    }
}
