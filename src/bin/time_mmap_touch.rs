//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

use std::ptr;

use bmk_linux::{
    resultarray::{ResultArray, PAGE_SIZE},
    timing::rdtsc,
};

use clap::clap_app;

use libc::{
    mmap as libc_mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_POPULATE, MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

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
        (@arg PREFAULT: -p --prefault "If present, the bmk will prefault memory before beginning.")
        (@arg PFTIME: --pftime +takes_value {is_int}
         "If present, does a hypercall toet the PF_TIME to the given value.")
        (@group pattern =>
            (@attributes +required)
            (@arg zeros: -z "Fill pages with zeros")
            (@arg counter: -c "Fill pages with counter values")
        )
        (@arg STATS_GB: --stats_gb {is_int} +takes_value
         "Amount of memory used to store stats (in GB).")
    }
    .get_matches();

    // How much memory for stats.
    let stats_gb = if let Some(gbs) = matches.value_of("STATS_GB") {
        gbs.parse::<usize>().unwrap()
    } else {
        16
    };

    // How many pages to touch?
    let npages = matches
        .value_of("SIZE")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap();
    let npages = npages - (stats_gb << 18); // subtract out space for stats

    // How many times to record stats (each measurement is 8B, 1GB total)?
    let nstats = (stats_gb << 30) / 8;

    // Frequency of recording stats (measure every freq-th operation).
    let freq = if npages < nstats {
        1
    } else {
        // We need to round up to account for a possible remainder.
        npages / nstats + 1
    };

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

    // Should we prefault?
    let prefault = matches.is_present("PREFAULT");

    ///////////////////////////////////////////////////////////////////////////
    // Start the experiment
    ///////////////////////////////////////////////////////////////////////////

    // Mmap memory for the experiment
    let mapped = unsafe {
        let addr = libc_mmap(
            ptr::null_mut(),
            npages * PAGE_SIZE,
            PROT_READ | PROT_WRITE,
            if prefault {
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_POPULATE
            } else {
                MAP_PRIVATE | MAP_ANONYMOUS
            },
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

    // Set the PF time.
    if let Some(pf_time) = matches.value_of("PFTIME") {
        let pf_time = pf_time.to_string().parse::<u64>().unwrap();
        paperexp::vmcall_pf_time(pf_time);
    }

    // Get initial timestamp
    let first = rdtsc();

    // Touch all memory
    for i in 0..npages {
        unsafe {
            *mapped.offset((i * PAGE_SIZE) as isize) = val;
        }

        // Maybe take a measurement
        if i % freq == 0 {
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
