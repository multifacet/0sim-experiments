//! Grabs a bunch of memory and sits on it. This is not really a benchmark but more of just a
//! utility.

use std::ptr;
use std::time::Duration;

use bmk_linux::resultarray::PAGE_SIZE;

use clap::clap_app;

use libc::{
    mmap as libc_mmap, MAP_ANONYMOUS, MAP_FAILED, MAP_POPULATE, MAP_PRIVATE, PROT_READ, PROT_WRITE,
};

fn is_int(arg: String) -> Result<(), String> {
    arg.to_string()
        .parse::<usize>()
        .map_err(|_| "Not a valid usize".to_owned())
        .map(|_| ())
}

fn main() {
    let matches = clap_app! { hog =>
        (@arg SIZE: +required {is_int} "The number of pages to hog")
    }
    .get_matches();

    // How many pages to touch?
    let npages = matches
        .value_of("SIZE")
        .unwrap()
        .to_string()
        .parse::<usize>()
        .unwrap();

    // Mmap memory for the experiment
    let _ = unsafe {
        let addr = libc_mmap(
            ptr::null_mut(),
            npages * PAGE_SIZE,
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

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
