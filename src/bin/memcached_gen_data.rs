//! Sits in a loop doing `put` operations on the given memcached instance. The keys are unique, but
//! the values are large, all-zero values.
//!
//! NOTE: This should be run from a machine that has a high-bandwidth, low-latency connection with
//! the test machine.
//!
//! NOTE: The server should be started with e.g. `memcached -M -m 50000` for 50GB.

use std::time::Instant;

use bmk_linux::timing::{Clock, Tsc};

use clap::clap_app;

use memcache::{Client, MemcacheError};

/// Print a measurement every `PRINT_INTERVAL`-th `put`
const PRINT_INTERVAL: usize = 100;

/// The TTL of the key/value pairs
const EXPIRATION: u32 = 1_000_000; // A really long time

/// The size of a single value in a key value pair. This is fine tuned so that there is no wasted
/// space if memcached is started with `-f 1.11`.
const VAL_SIZE: usize = 523800;

/// A big array that constitutes the values to be `put`
const ZEROS: &[u8] = &[0; VAL_SIZE];

fn is_addr(arg: String) -> Result<(), String> {
    use std::net::ToSocketAddrs;

    arg.to_socket_addrs()
        .map_err(|_| "Not a valid IP:Port".to_owned())
        .map(|_| ())
}

fn is_int(arg: String) -> Result<(), String> {
    arg.to_string()
        .parse::<usize>()
        .map_err(|_| "Not a valid usize".to_owned())
        .map(|_| ())
}

fn run<C: Clock>(
    addr: &str,
    nputs: usize,
    page_tables: bool,
    use_hypercall: bool,
    freq: usize,
) -> Result<(), MemcacheError> {
    // Connect to the kv-store
    let mut client = Client::new(format!("memcache://{}", addr).as_str())?;

    // First time stamp
    let mut time = C::now();

    // Actually put into the kv-store
    for i in 0..nputs {
        // `put`
        let mut res = client.set(&format!("{}", i), ZEROS, EXPIRATION);

        // HACK: Still not sure why things fail. So retry if we failed. That seems to work.
        while let Err(e) = res {
            println!("memcached returned error: {}", e);

            match e {
                MemcacheError::Io(ref err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                    return Err(e)
                }
                _ => {}
            }

            res = client.set(&format!("{}", i), ZEROS, EXPIRATION);
        }

        // periodically print
        if i % PRINT_INTERVAL == 0 {
            if page_tables {
                println!("DONE {} {}", i, paperexp::get_page_table_kbs());
            } else {
                let mut now = C::now();
                now.set_scaling_factor(freq);
                let diff = now.duration_since(time);
                let hypercall = if use_hypercall {
                    paperexp::vmcall_host_elapsed()
                } else {
                    0
                };
                println!(
                    "DONE {} Duration {{ secs: {}, nanos: {} }} {}",
                    i,
                    diff.as_secs(),
                    diff.subsec_nanos(),
                    hypercall
                );
                time = now;
            }
        }
    }

    Ok(())
}

fn main() {
    let matches = clap_app! { time_mmap_touch =>
        (@arg MEMCACHED: +required {is_addr}
         "The IP:PORT of the memcached instance")
        (@arg SIZE: +required {is_int}
         "The amount of data to put (in GB)")
        (@arg HYPERCALL: -h --hyperv
         "Pass this flag to use the hypercall")
        (@arg PAGE_TABLES: -p --page_tables
         "Pass this flag to measure page table overhead instead of latency")
        (@arg FREQ: -f --freq +takes_value {is_int}
         "Pass this flag to use `rdtsc` as the clock source. Use the given frequency \
          to convert clock ticks to seconds. The frequency should be stable (e.g. via \
          cpupower and pinning. Frequency should be an integer in MHz.")
        (@arg PFTIME: --pftime +takes_value {is_int}
         "If present, does a hypercall toet the PF_TIME to the given value.")
    }
    .get_matches();

    // Get the memcached addr
    let addr = matches.value_of("MEMCACHED").unwrap();

    // Get the amount of data to put
    let size = matches
        .value_of("SIZE")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap()
        << 30;

    // Total number of `put`s required
    let nputs = size / VAL_SIZE;

    // Check if we are to account for hypervisor
    let use_hypercall = matches.is_present("HYPERCALL");

    // Check if we are to measure page table overhead instead
    let page_tables = matches.is_present("PAGE_TABLES");
    assert!(!use_hypercall || !page_tables);

    let scaling_factor = if let Some(freq) = matches.value_of("FREQ") {
        freq.to_string().parse::<usize>().unwrap()
    } else {
        1
    };

    // Set the PF time.
    if let Some(pf_time) = matches.value_of("PFTIME") {
        let pf_time = pf_time.to_string().parse::<u64>().unwrap();
        paperexp::vmcall_pf_time(pf_time);
    }

    let result = if matches.is_present("FREQ") {
        run::<Tsc>(addr, nputs, page_tables, use_hypercall, scaling_factor)
    } else {
        run::<Instant>(addr, nputs, page_tables, use_hypercall, scaling_factor)
    };

    match result {
        Ok(()) => {}
        Err(e) => panic!("Error: {:?}", e),
    }
}
