//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

use bmk_linux::timing::rdtsc;

use clap::clap_app;

use libc::{
    mmap as libc_mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_POPULATE, MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

use rand::Rng;

fn is_usize(arg: String) -> Result<(), String> {
    arg.to_string()
        .parse::<usize>()
        .map_err(|_| "Not a valid usize".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app! { locality_mem_access =>
        (about: "Measures the latency difference in memory access patterns with strong and poor
                 spatial locality")
        (@group MODE =>
            (@attributes +required)
            (@arg local: -l "Access memory with strong locality")
            (@arg nonlocal: -n "Access memory with poor locality")
        )
        (@arg N: +required {is_usize}
         "The number of iterations (preferably divisible by 8).")
        (@arg MULTITHREAD: -t --threads +takes_value {is_usize}
         "(Optional) If passed with a value > 1, the bmk runs in multithreaded mode with the given \
         number of threads. Each thread gets it's own region of memory.")
    }
    .get_matches();

    let is_local = matches.is_present("local");

    let threads = matches
        .value_of("MULTITHREAD")
        .map(|value| value.parse().unwrap());

    let n = matches.value_of("N").unwrap().parse().unwrap();

    let ncpus = get_num_cpus();

    if let Some(threads) = threads {
        let mut handles = vec![];

        for i in 0..threads {
            handles.push(std::thread::spawn(move || do_work(is_local, i % ncpus, n)));
        }

        for handle in handles.into_iter() {
            handle.join().unwrap();
        }
    } else {
        // Single threaded
        do_work(is_local, 0, n);
    }
}

/// Actually do the work of the benchmark. Pin the work to the given cpu core.
fn do_work(is_local: bool, core: usize, n: usize) {
    // CPU pinning
    paperexp::set_cpu(core);

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

    // Warmup phase: 1 word of the first 8 pages.
    for i in 0..8 {
        unsafe {
            *mapped.offset((i << 12) as isize) = 7;
        }
    }

    if is_local {
        // Touch these warm cache lines a lot and time it
        for _ in 0..(n / 8) {
            for i in 0..8 {
                let start = rdtsc();
                unsafe {
                    *mapped.offset((i << 12) as isize) = 8;
                }
                let end = rdtsc();

                println!("{}", end - start);
            }
        }
    } else {
        let mut rng = rand::thread_rng();

        // Do something that has terrible performance
        // - lots of cache and TLB misses
        // - random behavior to avoid prefetchers
        for _ in 0..n {
            let i: isize = rng.gen_range(0, (4 << 30) >> 12);

            let start = rdtsc();
            unsafe {
                *mapped.offset(i << 12) = 9;
            }
            let end = rdtsc();

            println!("{}", end - start);
        }
    }
}

fn get_num_cpus() -> usize {
    let re = regex::Regex::new(r#"CPU\(s\):\s+(\d+)"#).unwrap();

    let out = std::process::Command::new("lscpu").output().unwrap();
    let out = std::str::from_utf8(&out.stdout).unwrap();

    let cpus = re
        .captures_iter(out)
        .next()
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .to_owned()
        .parse::<usize>()
        .unwrap();

    cpus
}
