use std::alloc;
use std::ptr::NonNull;

use crate::error::Error;

/// The underlying buffer which will manage grow, push, dealloc, etc.
pub struct RawBuffer {
    pub(crate) ptr: NonNull<u8>,
    pub(crate) capacity: usize,
    pub(crate) needs_drop: Option<unsafe fn(*mut u8, usize)>,
}

impl RawBuffer {
    pub const fn new(
        item_layout: &alloc::Layout,
        needs_drop: Option<unsafe fn(*mut u8, usize)>,
    ) -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: if item_layout.size() == 0 { usize::MAX } else { 0 },
            needs_drop,
        }
    }

    pub fn with_capacity(
        item_layout: &alloc::Layout,
        needs_drop: Option<unsafe fn(*mut u8, usize)>,
        capacity: usize,
    ) -> Self {
        let mut this = Self::new(item_layout, needs_drop);

        if capacity > 0 && this.capacity == 0 {
            this.grow(item_layout, capacity);
        }

        this
    }

    pub fn grow(&mut self, item_layout: &alloc::Layout, new_capacity: usize) {
        let size_t = item_layout.size();
        let align_t = item_layout.align();
        let new_size = size_t * new_capacity;

        let (layout, ptr) = if self.capacity == 0 {
            let layout = alloc::Layout::from_size_align(new_size, align_t).unwrap();
            let ptr = unsafe { alloc::alloc(layout) };

            (layout, ptr)
        } else {
            let old_size = size_t * self.capacity;
            let layout = alloc::Layout::from_size_align(old_size, align_t).unwrap();
            let ptr = unsafe { alloc::realloc(self.ptr.as_ptr(), layout, new_size) };

            (layout, ptr)
        };

        match NonNull::new(ptr) {
            Some(new) => {
                self.ptr = new;
                self.capacity = new_capacity;
            },
            None => alloc::handle_alloc_error(layout),
        }
    }

    pub const fn check(&self, offset: usize) -> Result<(), Error> {
        if self.capacity == 0 {
            return Err(Error::Uninitialized);
        } else if offset >= self.capacity {
            return Err(Error::ExceedCurrentCapacity);
        } else {
            Ok(())
        }
    }

    pub const fn cast<T>(&self) -> *mut T {
        self.ptr.cast::<T>().as_ptr()
    }

    pub const unsafe fn get_raw(&self, offset: usize) -> *mut u8 {
        unsafe {
            self.ptr.add(offset).as_ptr()
        }
    }

    pub const unsafe fn push<T>(&mut self, data: T, offset: usize) {
        if size_of::<T>() == 0 {
            unsafe {
                let ptr = self.cast::<T>();
                std::ptr::write(ptr, data);
                return;
            }
        }

        unsafe {
            let raw = self.cast::<T>().add(offset);
            std::ptr::write(raw, data);
        }
    }

    pub unsafe fn push_raw(
        &mut self,
        data: *mut u8,
        offset: usize,
        item_layout: &alloc::Layout
    ) {
        let size_t = item_layout.size();

        if size_t == 0 {
            unsafe {
                let ptr = self.ptr.as_ptr();
                std::ptr::copy(data.cast_const(), ptr, size_t);
            }
        } else {
            unsafe {
                let aligned_offset = self.ptr.align_offset(item_layout.align());
                let offset = aligned_offset + offset;
                let raw = self.ptr.add(offset * size_t);
                std::ptr::copy(data.cast_const(), raw.as_ptr(), size_t);
            }
        }

        unsafe {
            alloc::dealloc(data, *item_layout);
        }
    }

    /// this method already handle if index is equal to last_index or not -> swapping or popping
    pub unsafe fn swap_remove_or_pop(
        &mut self,
        index: usize,
        last_index: usize,
        size_t: usize,
    ) -> *mut u8 {
        if size_t == 0 {
            let ptr = std::ptr::without_provenance_mut(self.ptr.as_ptr() as usize + index);
            return ptr;
        }

        unsafe {
            let last = self.get_raw(last_index * size_t);

            if index < last_index {
                let to_remove = self.get_raw(index * size_t);
                std::ptr::swap_nonoverlapping(to_remove, last, size_t);
            }

            last
        }
    }

    pub fn clear(&mut self, len: usize) {
        let needs_drop = self.needs_drop.take();
        if let Some(drop_fn) = needs_drop {
            unsafe {
                drop_fn(self.ptr.as_ptr(), len)
            }
        }
        self.needs_drop = needs_drop;
    }

    pub fn dealloc(&mut self, item_layout: alloc::Layout) {
        let size_t = item_layout.size();

        if self.capacity > 0 && size_t > 0 {
            unsafe {
                let size = size_t * self.capacity;
                let align = item_layout.align();
                let layout = alloc::Layout::from_size_align_unchecked(size, align);
                alloc::dealloc(self.ptr.as_ptr(), layout);
            }
        }
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}
