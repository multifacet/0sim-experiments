//! Touch the given number of pages. Record the total time taken, peridically record elapsed time.
//! Fill the pages with the requested pattern.
//!
//! NOTE: all measurements are done with `rdtsc`, which reports cycle counts.

use bmk_linux::timing::rdtsc;

use paperexp::{vmcall_calibrate, vmcall_nop};

use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    const ACC: i64 = 100000;
    const EPSILON: i64 = 50;
    const NUM_BELOW_EP: usize = 50;

    let mut devnull = OpenOptions::new().write(true).open("/dev/null").unwrap();

    let mut tries = NUM_BELOW_EP;

    loop {
        let mut sum: i64 = 0;
        for _ in 0..ACC {
            let start = rdtsc() as i64;
            vmcall_nop();
            sum += rdtsc() as i64 - start;
            writeln!(devnull, "").unwrap();
        }

        let avg = sum / ACC;
        println!("avg {}", avg);
        let too_low = avg > 0;
        if avg.abs() > EPSILON {
            vmcall_calibrate(too_low);
        } else {
            if tries > 0 {
                tries -= 1;
            } else {
                break;
            }
        }
    }
}
