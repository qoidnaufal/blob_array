use std::marker::PhantomData;
use std::ptr::NonNull;

pub struct Iter<'a, T> {
    start: *mut T,
    end: *mut T,
    marker: PhantomData<&'a [T]>,
}

impl<'a, T> Iter<'a, T> {
    pub fn new(start: *mut T, len: usize) -> Self {
        let end = if size_of::<T>() == 0 {
            std::ptr::without_provenance_mut(start as usize + len)
        } else {
            unsafe { start.add(len) }
        };

        Self {
            start,
            end,
            marker: PhantomData,
        }
    }
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if size_of::<T>() == 0 {
                    self.start = std::ptr::without_provenance_mut(self.start as usize + 1);
                    Some(&*NonNull::<T>::dangling().as_ptr())
                } else {
                    let next = self.start;
                    self.start = next.add(1);
                    Some(&*next)
                }
            }
        }
    }
}

impl<'a, T: 'a> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if size_of::<T>() == 0 {
                    self.end = std::ptr::without_provenance_mut(self.end as usize - 1);
                    Some(&*NonNull::<T>::dangling().as_ptr())
                } else {
                    self.end = self.end.sub(1);
                    Some(&*self.end)
                }
            }
        }
    }
}

pub struct IterMut<'a, T> {
    start: *mut T,
    end: *mut T,
    marker: PhantomData<&'a mut [T]>,
}

impl<'a, T> IterMut<'a, T> {
    pub fn new(start: *mut T, len: usize) -> Self {
        let end = if size_of::<T>() == 0 {
            std::ptr::without_provenance_mut(start as usize + len)
        } else {
            unsafe { start.add(len) }
        };

        Self {
            start,
            end,
            marker: PhantomData,
        }
    }
}

impl<'a, T: 'a> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if size_of::<T>() == 0 {
                    self.start = std::ptr::without_provenance_mut(self.start as usize + 1);
                    Some(&mut *NonNull::<T>::dangling().as_ptr())
                } else {
                    let next = self.start;
                    self.start = next.add(1);
                    Some(&mut *next)
                }
            }
        }
    }
}

impl<'a, T: 'a> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if size_of::<T>() == 0 {
                    self.end = std::ptr::without_provenance_mut(self.end as usize - 1);
                    Some(&mut *NonNull::<T>::dangling().as_ptr())
                } else {
                    self.end = self.end.sub(1);
                    Some(&mut *self.end)
                }
            }
        }
    }
}
