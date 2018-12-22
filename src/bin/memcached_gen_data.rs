//! Sits in a loop doing `put` operations on the given memcached instance. The keys are unique, but
//! the values are large, all-zero values.
//!
//! NOTE: This should be run from a machine that has a high-bandwidth, low-latency connection with
//! the test machine.
//!
//! NOTE: The server should be started with e.g. `memcached -M -m 50000` for 50GB.

use std::time::Instant;

use clap::clap_app;

use memcache::{Client, MemcacheError};

/// Print a measurement every `PRINT_INTERVAL`-th `put`
const PRINT_INTERVAL: usize = 100;

/// The TTL of the key/value pairs
const EXPIRATION: u32 = 1_000_000; // A really long time

/// The order of magnitude of the size of the values
const VAL_ORDER: usize = 19; // 20 seems to give a "too large" error

/// 2^`VAL_ORDER`
const VAL_SIZE: usize = 1 << VAL_ORDER;

/// A big array that constitutes the values to be `put`
const ZEROS: &[u8] = &[0; VAL_SIZE];

/// Processor frequency (3.5GHz on seclab8)
const FREQ: usize = 3500;

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

type Timestamp = Instant;
//type Timestamp = paperexp::Tsc; // also uncomment set_freq

fn run() -> Result<(), MemcacheError> {
    let matches = clap_app! { time_mmap_touch =>
        (@arg MEMCACHED: +required {is_addr} "The IP:PORT of the memcached instance")
        (@arg SIZE: +required {is_int} "The amount of data to put (in GB)")
        (@arg HYPERCALL: -h --hyperv "Pass this flag to use the hypercall")
    (@arg PAGE_TABLES: -p --page_tables "Pass this flag to measure page table overhead instead of latency")
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

    // Connect to the kv-store
    let mut client = Client::new(format!("memcache://{}", addr).as_str())?;

    // First time stamp
    let mut time = Timestamp::now();

    // Actually put into the kv-store
    for i in 0..nputs {
        // `put`
        client.set(&format!("{}", i), ZEROS, EXPIRATION)?;

        // periodically print
        if i % PRINT_INTERVAL == 0 {
            if page_tables {
                println!("DONE {} {}", i, paperexp::get_page_table_kbs());
            } else {
                let now = Timestamp::now();
                //let mut now = Timestamp::now();
                //now.set_freq(FREQ);
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
    match run() {
        Ok(()) => {}
        Err(e) => panic!("Error: {:?}", e),
    }
}
