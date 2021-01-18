//! Sits in a loop doing `put` operations on the given MongoDB instance. The keys are unique, but
//! the values are large, all-zero values.
//!
//! NOTE: This should be run from a machine that has a high-bandwidth, low-latency connection with
//! the test machine.
//!
use futures::executor::block_on;
use std::time::Instant;

use bmk_linux::timing::{Clock, Tsc};

use mongodb::bson::doc;

use clap::clap_app;

use mongodb::{Client, options::ClientOptions};

/// Print a measurement every `PRINT_INTERVAL`-th `put`
const PRINT_INTERVAL: usize = 100;

/// The size of a single value in a key value pair. This is fine tuned so that there is no wasted
/// space if memcached is started with `-f 1.11`.
const VAL_SIZE: usize = 523800;

/// A big array that constitutes the values to be `put`
const DATA: &[u8] = &[0x41; VAL_SIZE];

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

async fn run<C: Clock>(
    addr: &str,
    nputs: usize,
    freq: usize,
) -> Result<(), mongodb::error::Error> {
    let client_options = ClientOptions::parse(format!("mongodb://{}", addr).as_str()).await?;
    let client = Client::with_options(client_options)?;

    let db = client.database("testdb");
    let collection = db.collection("test");

    let data_str = std::str::from_utf8(&DATA).unwrap();

    // First time stamp
    let mut time = C::now();

    // Put values into the DB
    for i in 0..nputs {
        let doc = doc!("id": format!("{}", i), "data": data_str);
        collection.insert_one(doc, None).await?;

        // periodically print
        if i % PRINT_INTERVAL == 0 {
            let mut now = C::now();
            now.set_scaling_factor(freq);
            let diff = now.duration_since(time);
            println!(
                "DONE {} Duration {{ secs: {}, nanos: {} }}",
                i,
                diff.as_secs(),
                diff.subsec_nanos(),
            );
            time = now;
        }
    }

    Ok(())
}

fn main() {
    let matches = clap_app! { time_mmap_touch =>
        (@arg MONGODB: +required {is_addr}
         "The IP:PORT of the memcached instance")
        (@arg SIZE: +required {is_int}
         "The amount of data to put (in GB)")
        (@arg FREQ: -f --freq +takes_value {is_int}
         "Pass this flag to use `rdtsc` as the clock source. Use the given frequency \
          to convert clock ticks to seconds. The frequency should be stable (e.g. via \
          cpupower and pinning. Frequency should be an integer in MHz.")
        (@arg PFTIME: --pftime +takes_value {is_int}
         "If present, does a hypercall toet the PF_TIME to the given value.")
    }
    .get_matches();

    let addr = matches.value_of("MONGODB").unwrap();
    let size = matches
        .value_of("SIZE")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap()
        << 30;

    let nputs = size / VAL_SIZE;

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
        block_on(run::<Tsc>(addr, nputs, scaling_factor))
    } else {
        block_on(run::<Instant>(addr, nputs, scaling_factor))
    };

    match result {
        Ok(()) => {}
        Err(e) => panic!("Error: {:?}", e),
    }
}
