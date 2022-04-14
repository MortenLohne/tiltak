use std::{cell, num::NonZeroU32};

pub struct Arena<T> {
    data: Box<[cell::RefCell<T>]>,
    next_index: cell::Cell<NonZeroU32>,
}
#[derive(PartialEq, Debug)]
pub struct Index {
    data: NonZeroU32,
}

impl Index {
    fn new(data: NonZeroU32) -> Self {
        Self { data }
    }
}

impl<T: Default> Arena<T> {
    pub fn new(capacity: usize) -> Self {
        let mut data_vec = Vec::with_capacity(capacity);
        while data_vec.len() < data_vec.capacity() {
            data_vec.push(cell::RefCell::new(T::default()));
        }
        Self {
            data: data_vec.into_boxed_slice(),
            next_index: cell::Cell::new(NonZeroU32::new(1).unwrap()),
        }
    }

    pub fn get(&self, index: &Index) -> cell::Ref<T> {
        self.data[index.data.get() as usize].borrow()
    }

    pub fn get_mut(&self, index: &mut Index) -> cell::RefMut<T> {
        self.data[index.data.get() as usize].borrow_mut()
    }

    pub fn add(&self, value: T) -> Index {
        let old_index_raw = self
            .next_index
            .replace(NonZeroU32::new(self.next_index.get().get() + 1).unwrap());
        let mut old_index = Index::new(old_index_raw);
        *self.get_mut(&mut old_index) = value;
        old_index
    }
}
