pub mod raw_buffer;
pub mod type_erased;
pub mod unmanaged;
pub mod iterator;

pub mod util {
    /// Helper function to create a function to drop the allocated elements.
    /// Return None if T doesn't need drop.
    pub const fn needs_drop<T>() -> Option<unsafe fn(*mut u8, usize)> {
        #[inline]
        unsafe fn drop<T>(raw: *mut u8, len: usize) {
            unsafe {
                std::ptr::slice_from_raw_parts_mut(raw.cast::<T>(), len).drop_in_place();
            }
        }

        if std::mem::needs_drop::<T>() {
            Some(drop::<T>)
        } else {
            None
        }
    }
}

pub mod error {
    #[derive(Debug)]
    pub enum Error {
        ExceedCurrentCapacity,
        Uninitialized,
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    impl std::error::Error for Error {}
}

#[cfg(test)]
mod test {
    use crate::type_erased::BlobArray;
    use crate::unmanaged::UnmanagedArray;

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
        let mut buffer = BlobArray::with_capacity::<Obj>(1);
        assert!(buffer.raw.needs_drop.is_some());

        let balo = Obj { name: "Balo".to_string(), age: 69 };
        let nunez = Obj { name: "Nunez".to_string(), age: 888 };
    
        buffer.push(balo);
        buffer.push(nunez);

        let get_mut = buffer.get_mut::<Obj>(1);
        assert!(get_mut.is_some());

        get_mut.unwrap().age = 666;
    
        let get = buffer.get::<Obj>(1);
        assert!(get.is_some_and(|obj| obj.age == 666));
    
        println!("{:?}", get.unwrap());
        println!("quitting");
    }

    #[test]
    fn remove() {
        let mut buffer = BlobArray::with_capacity::<Obj>(5);

        for i in 0..5 {
            buffer.push(Obj { name: i.to_string(), age: i as _ });
        }

        let to_remove = 1;
        let removed = buffer.swap_remove::<Obj>(to_remove);
        assert!(removed.is_some());

        let removed = removed.unwrap();
        assert!(removed.age == to_remove as _);
    }

    #[test]
    fn zst() {
        const CAP: usize = 10;
        #[derive(Debug, PartialEq)] struct Zst;

        let mut buffer = BlobArray::new::<Zst>();
        for _ in 0..CAP {
            buffer.push(Zst);
        }

        let first = buffer.get::<Zst>(0);
        let second = buffer.get::<Zst>(1);

        assert_eq!(first, second);

        buffer.iter::<Zst>().for_each(|zst| println!("{zst:?}"));

        let removed = buffer.pop::<Zst>();
        assert!(removed.is_some());
    }

    #[test]
    fn unmanaged() {
        let layout = std::alloc::Layout::new::<&'static str>();
        let drop_fn = crate::util::needs_drop::<&'static str>();

        let mut buffer = UnmanagedArray::with_capacity(layout, drop_fn, 2);

        buffer.push("Balo", 0);
        buffer.push("Nunez", 1);

        let balo = unsafe { buffer.get_unchecked::<&str>(0) };
        println!("{balo:?}");

        let nunez = unsafe { buffer.get_unchecked::<&str>(1) };
        println!("{nunez:?}");

        buffer.clear(2);
        drop(buffer);
    }
}
