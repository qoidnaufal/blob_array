use std::alloc;

use crate::error::Error;
use crate::raw_buffer::RawBuffer;
use crate::iterator::{Iter, IterMut};

/// Similar with [`TypeErasedArray`](crate::type_erased::TypeErasedArray) but the responsibility of keeping track on how many elements inside the array is delegated somewhere else.
/// This is done this way because usually this array will be paired with a Vec\<T\> for example
/// 
/// # Safety
/// - You have to manually keep track on the amount of the items you have pushed, so you can
/// - Manually [`clear`](UnmanagedArray::clear), by providing the number of elements, before dropping this.
/// - Getters method are unchecked, because the caller will do the check beforehand anyway.
///
/// Not calling clear before dropping may potentially cause a memory leak
pub struct UnmanagedArray {
    pub(crate) raw: RawBuffer,
    item_layout: alloc::Layout,
}

impl Drop for UnmanagedArray {
    fn drop(&mut self) {
        self.raw.dealloc(self.item_layout);
    }
}

impl UnmanagedArray {
    pub const fn new(
        item_layout: alloc::Layout,
        needs_drop: Option<unsafe fn(*mut u8, usize)>,
    ) -> Self {
        Self {
            raw: RawBuffer::new(&item_layout, needs_drop),
            item_layout,
        }
    }

    pub fn layout(&self) -> &alloc::Layout {
        &self.item_layout
    }

    pub fn with_capacity(
        item_layout: alloc::Layout,
        needs_drop: Option<unsafe fn(*mut u8, usize)>,
        capacity: usize,
    ) -> Self {
        let raw = RawBuffer::with_capacity(&item_layout, needs_drop, capacity);

        Self {
            raw,
            item_layout,
        }
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

    pub const unsafe fn get_unchecked_mut<'a, T>(&'a mut self, index: usize) -> &'a mut T {
        unsafe {
            &mut *self.get_unchecked_raw(index).cast()
        }
    }

    /// Safety: you have to ensure buffer is already initialized or the number of elements are within [`capacity`](Self::capacity) - 1
    pub const unsafe fn push_unchecked<T>(&mut self, data: T, offset: usize) {
        unsafe { self.raw.push(data, offset); }
    }

    /// # Safety
    /// This method assumes that buffer is already initialized via [`with_capacity`](Self::with_capacity).
    /// So it's safe to return the pointer to the allocated data, because there's no reallocation that will cause the pointer to be invalid.
    pub fn push_within_capacity<T>(&mut self, data: T, offset: usize) -> Result<(), Error> {
        self.raw.check(offset)?;
        unsafe { self.push_unchecked(data, offset) };

        Ok(())
    }

    pub fn push<T>(&mut self, data: T, offset: usize) {
        if let Err(_) = self.raw.check(offset) {
            let new_capacity = self.raw.capacity + 4;
            self.raw.grow(&self.item_layout, new_capacity);
        }

        unsafe {
            self.push_unchecked(data, offset);
        }
    }

    pub fn push_raw(&mut self, data: *mut u8, offset: usize) {
        if let Err(_) = self.raw.check(offset) {
            let new_capacity = self.raw.capacity + 4;
            self.raw.grow(&self.item_layout, new_capacity);
        }

        unsafe {
            self.raw.push_raw(data, offset, &self.item_layout);
        }
    }

    pub fn extend<T>(&mut self, offset: usize, len: usize, iter: impl IntoIterator<Item = T>) {
        let upper_offset = offset + len;
        if let Err(_) = self.raw.check(upper_offset) {
            let new_capacity = self.raw.capacity + len;
            self.raw.grow(&self.item_layout, new_capacity);
        }

        (offset..upper_offset).zip(iter.into_iter()).for_each(|(idx, data)| unsafe {
            self.push_unchecked(data, idx);
        });
    }

    pub fn swap_remove_raw(&mut self, index: usize, len: usize) -> Option<*mut u8> {
        if len > 0 {
            unsafe {
                let last_index = len - 1;
                let ptr = self.raw.swap_remove_or_pop(index, last_index, self.item_layout.size());
                return Some(ptr);
            }
        }

        None
    }

    pub fn swap_remove<R>(&mut self, index: usize, len: usize) -> Option<R> {
        self.swap_remove_raw(index, len)
            .map(|ptr| unsafe { ptr.cast::<R>().read() })
    }

    pub fn clear(&mut self, len: usize) {
        self.raw.clear(len);
    }

    pub fn iter<T>(&self, len: usize) -> Iter<'_, T> {
        Iter::new(self.raw.cast::<T>(), len)
    }

    pub fn iter_mut<T>(&mut self, len: usize) -> IterMut<'_, T> {
        IterMut::new(self.raw.cast::<T>(), len)
    }
}
