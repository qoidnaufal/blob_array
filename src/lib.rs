use std::alloc;
use std::mem;
use std::ptr::NonNull;
use std::num::NonZeroUsize;
use std::cell::UnsafeCell;
use std::marker::PhantomData;

/// Type erased data storage. This is slightly slower than normal `Vec<T>`,
/// but faster than `Vec<Box<dyn Any>>` and the data are guaranteed to be stored contiguously.
/// However, this has double the size (48) compared to a normal Vec (24) which comes from the need to carry additional informations.
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

    #[inline(always)]
    unsafe fn get_raw<T>(&self, index: usize) -> *mut u8 {
        debug_assert!(index < self.len);
        unsafe {
            self.block.add(index * size_of::<T>()).as_ptr()
        }
    }

    pub fn get<T>(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None }

        unsafe {
            let raw = self.get_raw::<T>(index);
            Some(&*raw.cast::<T>())
        }
    }

    pub fn get_mut<T>(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len { return None }

        unsafe {
            let raw = self.get_raw::<T>(index);
            Some(&mut *raw.cast::<T>())
        }
    }
    
    pub fn get_cell<T>(&self, index: usize) -> Option<&UnsafeCell<T>> {
        if index >= self.len { return None }
       
        unsafe {
            let raw = self.get_raw::<T>(index);
            let ptr = raw.cast::<UnsafeCell<T>>();
            Some(&*ptr)
        }
    }

    pub fn swap_remove<T>(&mut self, index: usize) -> Option<T> {
        if index >= self.len { return None }

        let last_index = self.len - 1;

        unsafe {
            let last = self.get_raw::<T>(last_index).cast::<T>();
            self.len -= 1;

            if index < last_index {
                let to_remove = self.get_raw::<T>(index).cast::<T>();
                std::ptr::swap_nonoverlapping(to_remove, last, 1);
                Some(last.read())
            } else {
                Some(last.read())
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
            .get_cell::<T>(self.next)
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
    
        let get = ba.get_cell::<Obj>(1).map(|cell| unsafe {
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

        let removed = removed.unwrap();
        assert!(removed.age == to_remove as _);
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

    #[test]
    fn zst() {
        const CAP: usize = 2;
        struct Zst;
        let mut ba = BlobArray::new::<Zst>(CAP);
        for _ in 0..CAP {
            ba.push(Zst);
        }

        unsafe {
            let first = ba.get_raw::<Zst>(0) as usize;
            let second = ba.get_raw::<Zst>(1) as usize;

            assert_eq!(first, second);
        }
    }

    #[test]
    fn speed() {
        struct NewObj {
            _name: String,
            _age: usize,
        }

        const NUM: usize = 1024 * 1024;

        let mut ba = BlobArray::new::<NewObj>(NUM);
        let now = std::time::Instant::now();
        for i in 0..NUM {
            ba.push(NewObj { _name: i.to_string(), _age: i });
        }
        println!("blob array push time for {NUM} objects: {:?}", now.elapsed());

        let mut vec: Vec<Box<dyn std::any::Any>> = Vec::with_capacity(NUM);
        let now = std::time::Instant::now();
        for i in 0..NUM {
            vec.push(Box::new(NewObj { _name: i.to_string(), _age: i }));
        }
        println!("vec push time for {NUM} objects: {:?}", now.elapsed());
    }
}

// struct ElementInfo {
//     layout: alloc::Layout,
//     type_id: std::any::TypeId,
// }
