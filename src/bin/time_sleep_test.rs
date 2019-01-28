use std::thread::sleep;
use std::time::Duration;
use std::collections::HashMap;

use bmk_linux::timing::rdtsc;

fn main() {
    const N: usize = 10000;
    const MS: u64 = 10;

    let mut measurements = vec![0; N];

    for measurement in measurements.iter_mut() {
        let start = rdtsc();

        sleep(Duration::from_millis(MS));

        let elapsed = rdtsc() - start;

        *measurement = elapsed;
    }

    let mut md = MemoizedData::new();

    let avg = md.avg(&measurements);
    let sd = md.sd(&measurements);

    println!("avg: {}", avg);
    println!("sd: {} ({}%)", sd,  sd / avg * 100.);
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

struct MemoizedData {
    cached_avg : Option<f64>,
    cached_sd: Option<f64>,
    cached_max: Option<f64>,

    cached_sorted: Option<Vec<u64>>,
    cached_percentiles: HashMap<usize, f64>,
}

impl MemoizedData {
    pub fn new() -> Self {
        MemoizedData {
            cached_avg: None,
            cached_sd: None,
            cached_max: None,

            cached_sorted: None,
            cached_percentiles: HashMap::new(),
        }
    }

    pub fn avg(&mut self, measurements: &[u64]) -> f64 {
        if let Some(avg) = self.cached_avg {
            return avg;
        }

        let n = measurements.len();
        let sum: u64 = measurements.iter().sum();

        let avg = (sum as f64) / (n as f64);

        self.cached_avg = Some(avg);

        avg
    }

    pub fn sd(&mut self, measurements: &[u64]) -> f64 {
        if let Some(sd) = self.cached_sd {
            return sd;
        }

        let avg = self.avg(measurements);
        let deviations_sq: f64 = measurements.iter().map(|&x| (x as f64 - avg).powi(2)).sum();
        let sd  = deviations_sq.sqrt();

        self.cached_sd = Some(sd);

        sd
    }

    fn sorted_data(&mut self, measurements: &[u64]) -> &Vec<u64> {
        if let Some(ref sorted) = self.cached_sorted {
            return sorted;
        }

        let mut clone = measurements.to_vec();
        clone.sort_unstable();
        self.cached_sorted = Some(clone);

        self.cached_sorted.as_ref().unwrap()
    }

    pub fn percentile(&mut self, measurements: &[u64], percentile: usize) -> f64 {
        assert!(percentile < 100);

        if let Some(&percentile) = self.cached_percentiles.get(&percentile) {
            return percentile;
        }

        let val = {
            let sorted = self.sorted_data(measurements);
            let idx = sorted.len() * percentile / 100;
            //println!("[debug] {} {}", percentile, idx);
            assert!(idx < sorted.len());

            sorted[idx] as f64
        };

        self.cached_percentiles.insert(percentile, val);

        val
    }

    pub fn permicrotile(&mut self, measurements: &[u64], permicrotile: usize) -> f64 {
        assert!(permicrotile < 1_000_000 && permicrotile > 990_000);

        if let Some(&permicrotile) = self.cached_percentiles.get(&permicrotile) {
            return permicrotile;
        }

        let val = {
            let sorted = self.sorted_data(measurements);
            let idx = sorted.len() * permicrotile / 1_000_000;
            //println!("[debug] {} {}", permicrotile, idx);
            assert!(idx < sorted.len());

            sorted[idx] as f64
        };

        self.cached_percentiles.insert(permicrotile, val);

        val
    }

    pub fn max(&mut self, measurements: &[u64]) -> f64 {
        if let Some(max) = self.cached_max {
            return max;
        }

        let val = {
            let sorted = self.sorted_data(measurements);
            *sorted.last().unwrap() as f64
        };

        self.cached_max = Some(val);

        val
    }
}
