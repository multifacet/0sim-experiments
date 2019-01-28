//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

use clap::clap_app;

use bmk_linux::timing::rdtsc;

fn is_int(arg: String) -> Result<(), String> {
    arg.to_string()
        .parse::<usize>()
        .map_err(|_| "Not a valid usize".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app! { time_loop =>
        (@arg N: +required {is_int} "The number of iterations")
    }
    .get_matches();

    // How many pages to touch?
    let n = matches
        .value_of("N")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap();

    // Results array
    let mut results = Vec::with_capacity(n);

    ///////////////////////////////////////////////////////////////////////////
    // Start the experiment
    ///////////////////////////////////////////////////////////////////////////

    // Touch all memory
    for _ in 0..n {
        results.push(rdtsc());
    }

    // Print results and final time stamp
    for ts in results.iter() {
        println!("{}", ts);
    }
}
