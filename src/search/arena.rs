use std::cell;

pub struct Arena<T> {
    data: Box<[cell::RefCell<T>]>,
    next_index: cell::Cell<u32>,
}

impl<T: Default> Arena<T> {
    pub fn new(capacity: usize) -> Self {
        let mut data_vec = Vec::with_capacity(capacity);
        while data_vec.len() < data_vec.capacity() {
            data_vec.push(cell::RefCell::new(T::default()));
        }
        Self {
            data: data_vec.into_boxed_slice(),
            next_index: cell::Cell::new(1),
        }
    }

    pub fn get(&self, index: u32) -> cell::Ref<T> {
        self.data[index as usize].borrow()
    }

    pub fn get_mut(&self, index: u32) -> cell::RefMut<T> {
        self.data[index as usize].borrow_mut()
    }

    pub fn add(&self, value: T) -> u32 {
        let old_index = self.next_index.replace(self.next_index.get() + 1);
        *self.get_mut(old_index) = value;
        old_index
    }
}