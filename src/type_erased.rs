use std::alloc;

use crate::error::Error;
use crate::raw_buffer::RawBuffer;
use crate::iterator::{Iter, IterMut};

/// This is equivalent to a Vec<Box\<dyn Any\>>, but without the need to Box the element on insertion.
pub struct BlobArray {
    pub(crate) raw: RawBuffer,
    pub(crate) len: usize,
    item_layout: alloc::Layout,
}

impl Drop for BlobArray {
    fn drop(&mut self) {
        self.clear();
        self.raw.dealloc(self.item_layout);
    }
}

impl BlobArray {
    pub const fn new<T>() -> Self {
        let item_layout = alloc::Layout::new::<T>();

        Self {
            raw: RawBuffer::new(&item_layout, crate::util::needs_drop::<T>()),
            len: 0,
            item_layout,
        }
    }

    pub fn with_capacity<T>(capacity: usize) -> Self {
        let item_layout = alloc::Layout::new::<T>();
        let block = RawBuffer::with_capacity(&item_layout, crate::util::needs_drop::<T>(), capacity);

        Self {
            raw: block,
            len: 0,
            item_layout,
        }
    }

    pub fn as_slice<T>(&self) -> &[T] {
        unsafe {
            std::slice::from_raw_parts(self.raw.cast::<T>().cast_const(), self.len)
        }
    }

    pub fn as_slice_mut<T>(&mut self) -> &mut[T] {
        unsafe {
            std::slice::from_raw_parts_mut(self.raw.cast::<T>(), self.len)
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn capacity(&self) -> usize {
        self.raw.capacity
    }

    /// Safety: you have to ensure buffer is already initialized or the number of elements are within [`capacity`](Self::capacity) - 1
    unsafe fn push_unchecked<T>(&mut self, data: T) {
        unsafe { self.raw.push(data, self.len) };
        self.len += 1;
    }

    /// # Safety
    /// This method assumes that buffer is already initialized via [`with_capacity`](Self::with_capacity).
    /// If you provided zero capacity on initialization, first push will return error.
    /// Since there will be no reallocation, it's safe to return the pointer to the allocated data.
    pub fn push_within_capacity<T>(&mut self, data: T) -> Result<(), Error> {
        self.raw.check(self.len)?;
        unsafe { self.push_unchecked(data) };

        Ok(())
    }

    pub fn push<T>(&mut self, data: T) {
        if let Err(_) = self.raw.check(self.len) {
            self.raw.grow(&self.item_layout, self.raw.capacity + 4);
        }

        unsafe {
            self.push_unchecked(data);
        }
    }

    pub fn extend<T>(&mut self, len: usize, iter: impl IntoIterator<Item = T>) {
        if let Err(_) = self.raw.check(self.len + len) {
            let new_capacity = self.raw.capacity + len;
            self.raw.grow(&self.item_layout, new_capacity);
        }

        iter.into_iter().for_each(|data| unsafe {
            self.push_unchecked(data);
        });
    }

    pub(crate) fn clear(&mut self) {
        self.raw.clear(self.len);
        self.len = 0;
    }

    pub const unsafe fn get_unchecked_raw(&self, index: usize) -> *mut u8 {
        unsafe {
            self.raw.get_raw(index * self.item_layout.size())
        }
    }

    pub const unsafe fn get_unchecked<'a, T>(&'a self, index: usize) -> &'a T {
        unsafe {
            &*self.get_unchecked_raw(index).cast()
        }
    }

    pub const fn get<'a, T>(&'a self, index: usize) -> Option<&'a T> {
        if index >= self.len { return None }

        unsafe {
            Some(self.get_unchecked(index))
        }
    }

    pub const unsafe fn get_unchecked_mut<'a, T>(&'a mut self, index: usize) -> &'a mut T {
        unsafe {
            &mut *self.get_unchecked_raw(index).cast()
        }
    }

    pub const fn get_mut<'a, T>(&'a mut self, index: usize) -> Option<&'a mut T> {
        if index >= self.len { return None }

        unsafe {
            Some(self.get_unchecked_mut(index))
        }
    }

    fn swr<R>(&mut self, index: usize, f: impl FnOnce(*mut u8) -> R) -> Option<R> {
        if self.len > 0 {
            unsafe {
                let last_index = self.len - 1;
                let ptr = self.raw.swap_remove_or_pop(index, last_index, self.item_layout.size());
                self.len -= 1;
                return Some(f(ptr));
            }
        }

        None
    }

    pub fn swap_remove<T>(&mut self, index: usize) -> Option<T> {
        self.swr::<T>(index, |raw| unsafe { raw.cast::<T>().read() })
    }

    pub fn swap_remove_and_drop<T>(&mut self, index: usize) {
        self.swr::<()>(index, |raw| unsafe { raw.cast::<T>().drop_in_place() });
    }

    pub fn pop<T>(&mut self) -> Option<T> {
        if self.len > 0 {
            unsafe {
                let last_index = self.len - 1;
                let ptr = self.raw.swap_remove_or_pop(last_index, last_index, self.item_layout.size());
                self.len -= 1;
                return Some(ptr.cast::<T>().read());
            }
        }

        None
    }

    pub fn iter<'a, T>(&'a self) -> Iter<'a, T> {
        Iter::new(self.raw.cast::<T>(), self.len())
    }

    pub fn iter_mut<'a, T>(&'a mut self) -> IterMut<'a, T> {
        IterMut::new(self.raw.cast::<T>(), self.len())
    }
}
