//! Sits in a loop doing `put` operations on the given memcached instance. The keys are unique, but
//! the values are large, all-zero values.
//!
//! In the meantime, every N seconds, it executes syscall 335 to get THP compaction stats, where N
//! is a command line arg. The results are printed to stdout.
//!
//! NOTE: This should be run from a machine that has a high-bandwidth, low-latency connection with
//! the test machine.
//!
//! NOTE: The server should be started with e.g. `memcached -M -m 50000` for 50GB.

#[macro_use]
extern crate clap;
extern crate memcache;
extern crate paperexp;

use std::time::Instant;

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
        (@arg INTERVAL: +required {is_int} "The amount of time between calls to the syscall")
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

    // Connect to the kv-store
    let mut client = Client::new(format!("memcache://{}", addr).as_str())?;

    // Interval to poll
    let interval = matches
        .value_of("INTERVAL")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap();

    // First time stamp
    let mut time = Timestamp::now();

    // Start a thread that does stuff
    let stop_flag = std::sync::atomic::AtomicBool::new(false);

    let measure_thread = std::thread(move || {
        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            // Sleep for a while
            std::thread::sleep(std::time::Duration::from_secs(interval));

            // Take a measurement
            let ops = paperexp::thp_compaction_syscall(paperexp::THPCompactionSyscallWhich::Ops);
            let undone =
                paperexp::thp_compaction_syscall(paperexp::THPCompactionSyscallWhich::UndoneOps);

            println!("{} {}", ops, undone);
        }
    });

    // Start doing insertions
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
                time = now;
            }
        }
    }

    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    measure_thread.join();

    Ok(())
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => panic!("Error: {:?}", e),
    }
}
