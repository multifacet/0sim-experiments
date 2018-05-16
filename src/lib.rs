//! Some utilities for experiments. These are mostly wrappers around libc.

#![feature(asm)]

extern crate libc;

use std::{
    mem, ops::{Index, IndexMut}, ptr, slice,
};

use libc::{
    mlock as libc_mlock, mmap as libc_mmap, munmap as libc_munmap, MAP_ANONYMOUS, MAP_FAILED,
    MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

/// Number of bytes in a page.
pub const PAGE_SIZE: usize = 1 << 12; // 4KB

/// A pre-allocated, mlocked, and prefaulted array of the given size and type for storing results.
/// This is useful to the storage of results from interfering with measurements.
pub struct ResultArray<T: Sized> {
    array: Vec<T>,
}

impl<T: Sized> ResultArray<T> {
    /// Create a new `ResultArray` with the given number of elements.
    ///
    /// # Panics
    ///
    /// - If unable to create the array.
    /// - If the size of the array is not a multiple of the page size.
    pub fn new(nelem: usize) -> Self {
        let size = nelem * mem::size_of::<T>();

        assert!(size % PAGE_SIZE == 0);

        // Get the virtual address space.
        let mapped = unsafe {
            let addr = libc_mmap(
                ptr::null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );

            if addr == MAP_FAILED {
                panic!("Unable to mmap");
            }

            addr as *mut _
        };

        // Populate and lock the whole array
        unsafe {
            let ret = libc_mlock(mapped as *const _, size);
            assert_eq!(ret, 0);
        }

        Self {
            array: unsafe { Vec::from_raw_parts(mapped, 0, nelem) },
        }
    }

    pub fn iter(&self) -> slice::Iter<T> {
        self.array.iter()
    }
}

impl<T: Sized> Drop for ResultArray<T> {
    fn drop(&mut self) {
        // Drain the vec
        drop(self.array.drain(0..));

        // munmap
        let size = self.array.capacity() * mem::size_of::<T>();
        let ptr = self.array.as_mut_ptr();

        mem::forget(self); // never call `Vec::drop`

        unsafe {
            libc_munmap(ptr as *mut _, size);
        }
    }
}

impl<T: Sized> Index<usize> for ResultArray<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.array[index]
    }
}

impl<T: Sized> IndexMut<usize> for ResultArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.array[index]
    }
}

/// Run the `rdtsc` instruction and return the value
#[inline(always)]
pub fn rdtsc() -> u64 {
    let hi: u32;
    let lo: u32;

    unsafe {
        asm!("rdtsc" : "={eax}"(lo), "={edx}"(hi));
    }

    lo as u64 | ((hi as u64) << 32)
}
