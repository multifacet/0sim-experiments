use std::{thread::sleep, time::Duration};

use bmk_linux::timing::{rdtsc, MemoizedTimingData};

fn main() {
    let measurements = match std::env::args().skip(1).next().as_ref().map(|s| s.as_str()) {
        Some("sleep") => sleep_ms(),
        Some("nop") => sleep_nop(),
        Some("lock") => sleep_lock(),
        Some(other) => panic!("unexpected value \"{}\"", other),
        None => panic!("expected bmk name"),
    };

    let mut md = MemoizedTimingData::new();

    let avg = md.avg(&measurements);
    let sd = md.sd(&measurements);

    println!("avg: {}", avg);
    println!("sd: {} ({}%)", sd, sd / avg * 100.);
    println!("50%: {}", md.percentile(&measurements, 50));
    println!("75%: {}", md.percentile(&measurements, 75));
    println!("90%: {}", md.percentile(&measurements, 90));
    println!("99%: {}", md.percentile(&measurements, 99));
    println!("99.9%: {}", md.permicrotile(&measurements, 999_000));
    println!("99.99%: {}", md.permicrotile(&measurements, 999_900));
    println!("99.999%: {}", md.permicrotile(&measurements, 999_990));
    println!("99.9999%: {}", md.permicrotile(&measurements, 999_999));
    println!("max: {}", md.max(&measurements));
}

fn sleep_ms() -> Vec<u64> {
    const N: usize = 10000;
    const MS: u64 = 10;

    let mut measurements = vec![0; N];

    for measurement in measurements.iter_mut() {
        let start = rdtsc();

        sleep(Duration::from_millis(MS));

        let elapsed = rdtsc() - start;

        *measurement = elapsed;
    }

    measurements
}

fn sleep_nop() -> Vec<u64> {
    const N: usize = 100_000_000;

    let mut measurements = vec![0; N];

    for measurement in measurements.iter_mut() {
        let start = rdtsc();

        let elapsed = rdtsc() - start;

        *measurement = elapsed;
    }

    measurements
}

fn sleep_lock() -> Vec<u64> {
    const N: usize = 100_000_000;

    let mut measurements = vec![0; N];

    let lock = std::sync::Mutex::new(());

    for measurement in measurements.iter_mut() {
        let start = rdtsc();

        {
            let _ = lock.lock().unwrap();
        }

        let elapsed = rdtsc() - start;

        *measurement = elapsed;
    }

    measurements
}
