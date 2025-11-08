use std::alloc;
use std::mem;
use std::ptr::NonNull;
use std::num::NonZeroUsize;
use std::cell::UnsafeCell;
use std::marker::PhantomData;

/// Type erased data storage
pub struct BlobArray {
    block: NonNull<u8>,
    len: usize,
    capacity: NonZeroUsize,
    item_layout: alloc::Layout,
    drop: Option<unsafe fn(*mut u8, usize)>,
}

impl Drop for BlobArray {
    fn drop(&mut self) {
        unsafe {
            self.clear();
            let size = self.item_layout.size() * self.capacity.get();
            let align = self.item_layout.align();
            let layout = alloc::Layout::from_size_align_unchecked(size, align);
            alloc::dealloc(self.block.as_ptr(), layout);
        }
    }
}

impl BlobArray {
    // TODO: handle zero sized type
    pub fn new<T>(capacity: usize) -> Self {
        #[inline]
        unsafe fn drop<T>(raw: *mut u8, len: usize) {
            unsafe {
                let ptr = raw.cast::<T>();
                for i in 0..len {
                    let to_drop = ptr.add(i);
                    std::ptr::drop_in_place(to_drop);
                }
            }
        }

        let capacity = NonZeroUsize::try_from(capacity).unwrap();
        let size = size_of::<T>();
        let align = align_of::<T>();

        unsafe {
            let layout = alloc::Layout::from_size_align_unchecked(size * capacity.get(), align);
            let raw = std::alloc::alloc(layout);

            if raw.is_null() {
                alloc::handle_alloc_error(layout);
            }

            Self {
                block: NonNull::new_unchecked(raw),
                len: 0,
                capacity,
                item_layout: alloc::Layout::from_size_align_unchecked(size, align),
                drop: mem::needs_drop::<T>().then_some(drop::<T>),
            }
        }
    }

    pub fn push<T>(&mut self, data: T) {
        let size = size_of::<T>();
        let align = align_of::<T>();
        let capacity = self.capacity.get();

        if self.len == capacity {
            self.realloc(capacity + 1);
        }

        unsafe {
            let raw = self.block.add(self.len * size);
            let aligned = raw.align_offset(align);
            let ptr = raw.add(aligned).as_ptr();
            std::ptr::write(ptr.cast::<T>(), data);
        }

        self.len += 1;
    }

    fn realloc(&mut self, new_capacity: usize) {
        unsafe {
            let new_size = self.item_layout.size() * new_capacity;
            let new_block = alloc::realloc(self.block.as_ptr(), self.item_layout, new_size);

            self.block = NonNull::new_unchecked(new_block);
            self.capacity = NonZeroUsize::try_from(new_capacity).unwrap();
        }
    }

    unsafe fn get_raw<T>(&self, index: usize) -> *mut T {
        debug_assert!(index < self.len);
        unsafe {
            let raw = self.block.add(index * size_of::<T>());
            raw.as_ptr().cast::<T>()
        }
    }
    
    pub fn try_get<T>(&self, index: usize) -> Option<&UnsafeCell<T>> {
        if index >= self.len { return None }
       
        unsafe {
            let raw = self.block.add(index * size_of::<T>());
            let ptr = raw.as_ptr().cast::<UnsafeCell<T>>();
            Some(&*ptr)
        }
    }

    pub fn swap_remove<T>(&mut self, index: usize) -> Option<Ptr<T>> {
        if index >= self.len { return None }

        let last_index = self.len - 1;

        unsafe {
            let last = self.get_raw::<T>(last_index);
            self.len -= 1;

            if index < last_index {
                let to_remove = self.get_raw::<T>(index);
                std::ptr::swap_nonoverlapping(to_remove, last, 1);
                Some(Ptr::new(last))
            } else {
                Some(Ptr::new(last))
            }
        }
    }

    pub fn iter<'a, T>(&'a self) -> Iter<'a, T> {
        Iter::new(self)
    }

    pub fn clear(&mut self) {
        if let Some(drop) = self.drop {
            self.drop = None;
            unsafe { drop(self.block.as_ptr(), self.len) }
            self.drop = Some(drop);
            self.len = 0;
        }
    }
}

pub struct Ptr<T> {
    raw: NonNull<T>,
}

impl<T> Drop for Ptr<T> {
    fn drop(&mut self) {
        unsafe {
            self.raw.drop_in_place();
        }
    }
}

impl<T> Ptr<T> {
    fn new(raw: *mut T) -> Self {
        Self {
            raw: unsafe { NonNull::new_unchecked(raw) },
        }
    }

    pub fn read(self) -> T {
        unsafe {
            self.raw.read()
        }
    }
}

impl<T> std::ops::Deref for Ptr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            self.raw.as_ref()
        }
    }
}

impl<T> std::ops::DerefMut for Ptr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            self.raw.as_mut()
        }
    }
}

pub struct Iter<'a, T> {
    source: &'a BlobArray,
    next: usize,
    marker: PhantomData<UnsafeCell<T>>,
}

impl<'a, T> Iter<'a, T> {
    fn new(source: &'a BlobArray) -> Self {
        Self {
            source,
            next: 0,
            marker: PhantomData,
        }
    }
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = &'a UnsafeCell<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.source
            .try_get::<T>(self.next)
            .inspect(|_| self.next += 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    struct Obj {
        name: String,
        age: u32,
    }

    impl Drop for Obj {
        fn drop(&mut self) {
            println!("dropping {} aged {}", self.name, self.age)
        }
    }

    #[test]
    fn push_and_get() {
        let mut ba = BlobArray::new::<Obj>(1);
        assert!(ba.drop.is_some());

        let balo = Obj { name: "Balo".to_string(), age: 69 };
        let nunez = Obj { name: "Nunez".to_string(), age: 888 };
    
        ba.push(balo);
        ba.push(nunez);
    
        let get = ba.try_get::<Obj>(1).map(|cell| unsafe {
            let raw = cell.get();
            let this = &mut *raw;
            this.age = 0;
            &*raw
        });

        assert!(get.is_some_and(|obj| obj.age == 0));
    
        println!("{:?}", get.unwrap());
        println!("quitting");
    }

    #[test]
    fn remove() {
        let mut ba = BlobArray::new::<Obj>(5);

        for i in 0..5 {
            ba.push(Obj { name: i.to_string(), age: i as _ });
        }

        let to_remove = 1;
        let removed = ba.swap_remove::<Obj>(to_remove);
        assert!(removed.is_some());

        // let removed = removed.unwrap().read();
        // assert!(removed.age == to_remove as _);
    }

    #[test]
    fn iter() {
        let mut ba = BlobArray::new::<Obj>(5);

        for i in 0..5 {
            ba.push(Obj { name: i.to_string(), age: i as _ });
        }

        let iter = ba.iter::<Obj>();
        iter.for_each(|cell| unsafe {
            let obj = &mut *cell.get();
            obj.age = 0;
        });

        let mut iter2 = ba.iter::<Obj>();
        assert!(iter2.all(|cell| unsafe {
            let obj = &*cell.get();
            obj.age == 0
        }))
    }
}
