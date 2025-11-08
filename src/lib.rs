use std::ptr::NonNull;
use std::alloc;
use std::num::NonZeroUsize;
use std::cell::UnsafeCell;
use std::mem;

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

            let block = NonNull::new_unchecked(raw);

            Self {
                block,
                len: 0,
                capacity,
                item_layout: alloc::Layout::from_size_align_unchecked(size, align),
                drop: mem::needs_drop::<T>().then_some(drop::<T>),
            }
        }
    }

    pub fn alloc<T>(&mut self, data: T) {
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
    
    pub fn try_get<T>(&self, index: usize) -> Option<&UnsafeCell<T>> {
        if index >= self.len { return None }
       
        unsafe {
            let raw = self.block.add(index * size_of::<T>());
            let ptr = raw.as_ptr().cast::<UnsafeCell<T>>();
            Some(&*ptr)
        }
    }

    pub fn clear(&mut self) {
        if let Some(drop) = self.drop {
            self.drop = None;
            unsafe { drop(self.block.as_ptr(), self.len) }
            self.drop = Some(drop);
        }
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
    fn test_alloc() {
        let mut ba = BlobArray::new::<Obj>(1);
        assert!(ba.drop.is_some());

        let balo = Obj { name: "Balo".to_string(), age: 69 };
        let nunez = Obj { name: "Nunez".to_string(), age: 888 };
    
        ba.alloc(balo);
        ba.alloc(nunez);
    
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
}
