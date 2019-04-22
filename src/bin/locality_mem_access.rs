//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

use bmk_linux::timing::rdtsc;

use libc::{
    mmap as libc_mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_POPULATE, MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

use rand::Rng;

fn main() {
    // Mmap memory for the experiment
    let mapped = unsafe {
        let addr = libc_mmap(
            std::ptr::null_mut(),
            4 << 30,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_POPULATE,
            -1,
            0,
        );

        if addr == MAP_FAILED {
            panic!("Unable to mmap");
        }

        addr as *mut u8
    };

    // Warmup for first phase: touch the first 32kb of the memory (8 pages)
    for i in 0..8 {
        unsafe {
            *mapped.offset((i << 12) as isize) = 7;
        }
    }

    // Now touch these a lot and time it
    for _ in 0..10000 {
        for i in 0..8 {
            let start = rdtsc();
            unsafe {
                *mapped.offset((i << 12) as isize) = 8;
            }
            let end = rdtsc();

            println!("{}", end - start);
        }
    }

    println!("========");

    let mut rng = rand::thread_rng();

    // Now do something that has terrible performance
    // - lots of cache and TLB misses
    // - random behavior to avoid prefetchers
    for _ in 0..80000 {
        let i: isize = rng.gen_range(0, (4 << 30) >> 12);

        let start = rdtsc();
        unsafe {
            *mapped.offset(i << 12) = 9;
        }
        let end = rdtsc();

        println!("{}", end - start);
    }
}
