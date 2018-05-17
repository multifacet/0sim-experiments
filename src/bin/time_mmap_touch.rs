//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

#[macro_use]
extern crate clap;
extern crate libc;
extern crate paperexp;

use std::ptr;

use libc::{mmap as libc_mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE};

use paperexp::{rdtsc, ResultArray, PAGE_SIZE};

/// Either all zeros or counter values
enum Pattern {
    Zeros,
    Counter,
}

fn is_int(arg: String) -> Result<(), String> {
    arg.to_string()
        .parse::<usize>()
        .map_err(|_| "Not a valid usize".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app! { time_mmap_touch =>
        (@arg SIZE: +required {is_int} "The number of pages to touch")
        (@group pattern =>
            (@attributes +required)
            (@arg zeros: -z "Fill pages with zeros")
            (@arg counter: -c "Fill pages with counter values")
        )
    }.get_matches();

    // How many pages to touch?
    let npages = matches
        .value_of("SIZE")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap();

    // How many times to record stats (each measurement is 8B, 1GB total)?
    let nstats = (1 << 30) / 8;

    // Frequency of recording stats (measure every freq-th operation)
    let freq = if npages < nstats { 1 } else { npages / nstats };

    // Results array
    let mut results = ResultArray::new(nstats);

    // What pattern to use?
    let pattern = if matches.is_present("zeros") {
        Pattern::Zeros
    } else if matches.is_present("counter") {
        Pattern::Counter
    } else {
        unreachable!()
    };

    ///////////////////////////////////////////////////////////////////////////
    // Start the experiment
    ///////////////////////////////////////////////////////////////////////////

    // Mmap memory for the experiment
    let mapped = unsafe {
        let addr = libc_mmap(
            ptr::null_mut(),
            npages * PAGE_SIZE,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        );

        if addr == MAP_FAILED {
            panic!("Unable to mmap");
        }

        addr as *mut u8
    };

    // The value to fill memory with
    let mut val = 0;

    // Get initial timestamp
    let first = rdtsc();

    // Touch all memory
    for i in 0..npages {
        unsafe {
            *mapped.offset((i * PAGE_SIZE) as isize) = val;
        }

        // Maybe take a measurement
        if freq > 0 && i % freq == 0 {
            results.push(rdtsc());
        }

        // Update val
        val = match pattern {
            Pattern::Zeros => val,
            Pattern::Counter => val + 1,
        };
    }

    // Print results and final time stamp
    let last = rdtsc();
    println!("First: {}", first);
    println!("Last: {}", last);

    for ts in results.iter() {
        println!("{}", ts);
    }
}
