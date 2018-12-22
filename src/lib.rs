//! Some utilities for experiments. These are mostly wrappers around libc.

#![feature(asm)]

extern crate libc;
extern crate regex;

use std::{mem, ptr, slice, time::Duration};

use libc::{
    mlock as libc_mlock, mmap as libc_mmap, munmap as libc_munmap, MAP_ANONYMOUS, MAP_FAILED,
    MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

/// Number of bytes in a page.
pub const PAGE_SIZE: usize = 1 << 12; // 4KB

/// A pre-allocated, mlocked, and prefaulted array of the given size and type for storing results.
/// This is useful to the storage of results from interfering with measurements.
pub struct ResultArray<T: Sized> {
    array: Option<Vec<T>>,
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
            array: unsafe { Some(Vec::from_raw_parts(mapped, 0, nelem)) },
        }
    }

    pub fn iter(&self) -> slice::Iter<T> {
        self.array.as_ref().unwrap().iter()
    }

    pub fn push(&mut self, item: T) {
        self.array.as_mut().unwrap().push(item);
    }
}

impl<T: Sized> Drop for ResultArray<T> {
    fn drop(&mut self) {
        // Drain the vec
        drop(self.array.as_mut().unwrap().drain(0..));

        // munmap
        let mut array = self.array.take().unwrap();
        let size = array.capacity() * mem::size_of::<T>();
        let ptr = array.as_mut_ptr();

        mem::forget(array); // never call `Vec::drop`

        unsafe {
            libc_munmap(ptr as *mut _, size);
        }
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

/// The host elapsed time hypercall number.
const HV_GET_HOST_ELAPSED: u64 = 0x9;

/// The host nop hypercall number.
const HV_NOP: u64 = 0xA;

/// The host elapsed time calibration hypercall number.
const HV_CALIBRATE: u64 = 0xB;

/// Run the `vmcall 0x0009` instruction and return the value
#[inline(always)]
pub fn vmcall_host_elapsed() -> u64 {
    let hi: u32;
    let lo: u32;

    unsafe {
        asm!("
		mov $$0, %edx
		vmcall"
		: "={eax}"(lo), "={edx}"(hi)
		: "{eax}"(HV_GET_HOST_ELAPSED)
		:
		: "volatile");
    }

    lo as u64 | ((hi as u64) << 32)
}

/// Run the `vmcall 0x000A` instruction
#[inline(always)]
pub fn vmcall_nop() {
    unsafe {
        asm!("
		vmcall"
		:
		: "{eax}"(HV_NOP)
		:
		: "volatile");
    }
}

/// Run the `vmcall 0x000B` instruction and with the given value
#[inline(always)]
pub fn vmcall_calibrate(too_low: bool) {
    unsafe {
        asm!("
		vmcall"
		:
		: "{eax}"(HV_CALIBRATE), "{rbx}"(if too_low { 1 } else { 0 })
		:
		: "volatile");
    }
}

/// Like std::time::Instant but for rdtsc.
pub struct Tsc {
    tsc: u64,
    freq: Option<usize>,
}

impl Tsc {
    /// Capture the TSC now.
    pub fn now() -> Self {
        Tsc {
            tsc: rdtsc(),
            freq: None,
        }
    }

    /// Set the frequency of this `Tsc`. You need to do this before using `duration_since`;
    /// otherwise, we have no way to convert to seconds. `freq` should be in MHz.
    pub fn set_freq(&mut self, freq: usize) {
        self.freq = Some(freq);
    }

    /// Returns a `Duration` representing the time since `earlier`.
    ///
    /// # Panics
    ///
    /// If `earlier` is not `earlier`.
    pub fn duration_since(&self, earlier: Self) -> Duration {
        assert!(earlier.tsc < self.tsc);

        let diff = self.tsc - earlier.tsc;
        let nanos = diff * 1000 / self.freq.unwrap() as u64;

        Duration::from_nanos(nanos)
    }
}

const RE: &str = r"PageTables:\s*(\d+) kB";

pub fn get_page_table_kbs() -> usize {
    let data = std::fs::read_to_string("/proc/meminfo").unwrap();
    let re = regex::Regex::new(RE).unwrap();
    let caps = re.captures(&data).unwrap();
    caps.get(1).unwrap().as_str().parse::<usize>().unwrap()
}

pub enum THPCompactionSyscallWhich {
    Ops,
    UndoneOps,
}

pub const THP_COMPACTION_SYSCALL_NR: libc::c_long = 335;

/// Call syscall 335 to get the number of THP compaction operations that were done and undone.
pub fn thp_compaction_syscall(which: THPCompactionSyscallWhich) -> isize {
    unsafe {
        libc::syscall(
            THP_COMPACTION_SYSCALL_NR,
            match which {
                THPCompactionSyscallWhich::Ops => 0,
                THPCompactionSyscallWhich::UndoneOps => 1,
            },
        ) as isize
    }
}
