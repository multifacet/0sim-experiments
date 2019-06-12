//! Sits in a loop doing `put` operations on the given memcached instance. The keys are unique, but
//! the values are large, all-zero values.
//!
//! We do N insertions, followed by N/3 deletions, followed by N/2 more insertions.
//!
//! In the meantime, every N seconds, it executes syscall 335 to get THP compaction stats, where N
//! is a command line arg. The results are printed to stdout.
//!
//! NOTE: This should be run from a machine that has a high-bandwidth, low-latency connection with
//! the test machine.
//!
//! NOTE: The server should be started with e.g. `memcached -m 50000` for 50GB.

use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use bmk_linux::timing::rdtsc;

use clap::clap_app;

use memcache::Client;

use paperexp::CompactInstrumentationStats;

/// The TTL of the key/value pairs
const EXPIRATION: u32 = 1_000_000; // A really long time

/// The order of magnitude of the size of the values
const VAL_ORDER: usize = 19; // 20 seems to give a "too large" error

/// 2^`VAL_ORDER`
const VAL_SIZE: usize = 1 << VAL_ORDER;

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

macro_rules! try_again {
    ($e:expr) => {{
        if let Err(e) = $e {
            println!("unexpected error: {:?}", e);
            if let Err(e) = $e {
                println!("unexpected error: {:?}", e);
                return;
            }
        }
    }};
}

fn run() {
    let matches = clap_app! { time_mmap_touch =>
        (@arg MEMCACHED: +required {is_addr} "The IP:PORT of the memcached instance")
        (@arg SIZE: +required {is_int} "The amount of data to put (in GB)")
        (@arg INTERVAL: +required {is_int} "The interval at which to read compaction stats")
        (@arg OUTFILE: +required "The location to write memcached performance measurements to")
        (@arg CONTINUAL: --continual_compaction "Continually trigger compaction")
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
    let mut client = Client::new(format!("memcache://{}", addr).as_str()).unwrap();

    // Interval to poll
    let interval = matches
        .value_of("INTERVAL")
        .unwrap()
        .to_string()
        .parse::<u64>()
        .unwrap();

    let memcached_latency_file = matches.value_of("OUTFILE").unwrap();

    let continual_compaction = matches.is_present("CONTINUAL");

    // Start a thread that does stuff
    let stop_flag = Arc::new(AtomicBool::new(false));

    let measure_thread = {
        let stop_flag = Arc::clone(&stop_flag);

        std::thread::spawn(move || {
            let mut prev = 0;
            loop {
                // Sleep for a while
                std::thread::sleep(Duration::from_secs(interval));

                // Take a measurement
                let CompactInstrumentationStats { ops, undos } =
                    paperexp::thp_compact_instrumentation();

                // once the flag is set, wait to stabilize...
                if stop_flag.load(Ordering::Relaxed) {
                    if ops == prev {
                        break;
                    }
                }

                prev = ops;

                println!("{} {}", ops, undos);
            }
        })
    };

    // A thread that triggers continual compaction
    let compact_thread = if continual_compaction {
        let stop_flag = Arc::clone(&stop_flag);

        Some(std::thread::spawn(move || {
            loop {
                // Take a measurement
                paperexp::trigger_compaction(512).expect("trigger compaction failed");

                // once the flag is set, exit
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
            }
        }))
    } else {
        None
    };

    // Open a file for the latency measurements
    let memcached_latency_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(memcached_latency_file)
        .unwrap();
    let mut memcached_latency_file = BufWriter::new(memcached_latency_file);

    // Do the work.
    for i in 0..nputs {
        let start = rdtsc();

        // `put`
        try_again!(client.set(&format!("{}", i), ZEROS, EXPIRATION));

        write!(memcached_latency_file, "{}\n", rdtsc() - start).unwrap();
    }

    println!("NEXT!");
    writeln!(memcached_latency_file, "NEXT!").unwrap();

    // delete a third of previously inserted keys (they are random because memcached is a hashmap).
    for i in 0..nputs / 3 {
        let start = rdtsc();

        try_again!(client.delete(&format!("{}", i)));

        write!(memcached_latency_file, "{}\n", rdtsc() - start).unwrap();
    }

    println!("NEXT!");
    writeln!(memcached_latency_file, "NEXT!").unwrap();

    // insert more keys
    for i in nputs..(nputs + nputs / 2) {
        let start = rdtsc();

        // `put`
        try_again!(client.set(&format!("{}", i), ZEROS, EXPIRATION));

        write!(memcached_latency_file, "{}\n", rdtsc() - start).unwrap();
    }

    println!("DONE!");

    stop_flag.store(true, Ordering::Relaxed);

    measure_thread.join().unwrap();
    if let Some(compact_thread) = compact_thread {
        compact_thread.join().unwrap();
    }
}

fn main() {
    run();
}
