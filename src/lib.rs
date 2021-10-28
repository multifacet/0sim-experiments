//! Some utilities for experiments. These are mostly wrappers around libc.

#![feature(asm)]

/// The host elapsed time hypercall number.
const HV_GET_HOST_ELAPSED: u32 = 0x9;

/// The host nop hypercall number.
const HV_NOP: u32 = 0xA;

/// The host elapsed time calibration hypercall number.
const HV_CALIBRATE: u32 = 0xB;

/// The host elapsed time calibration hypercall number.
const HV_PF_TIME: u32 = 0xC;

/// Run the `vmcall 0x0009` instruction and return the value
#[inline(always)]
pub fn vmcall_host_elapsed() -> u64 {
    let hi: u32;
    let lo: u32;

    unsafe {
        asm!(
            "mov edx, eax
             vmcall",
             inout("eax") HV_GET_HOST_ELAPSED => lo,
             out("edx") hi,
        );
    }

    lo as u64 | ((hi as u64) << 32)
}

/// Run the `vmcall 0x000A` instruction
#[inline(always)]
pub fn vmcall_nop() {
    unsafe {
        asm!("vmcall", in("eax") HV_NOP);
    }
}

/// Run the `vmcall 0x000B` instruction and with the given value
#[inline(always)]
pub fn vmcall_calibrate(too_low: bool) {
    let too_low = if too_low { 1 } else { 0 };
    unsafe {
        asm!("
            push rbx
            mov rbx, {:r}
            vmcall
            pop rbx
            ",
            in(reg) too_low,
            in("eax") HV_CALIBRATE,
        );
    }
}

/// Run the `vmcall 0x000C` instruction
#[inline(always)]
pub fn vmcall_pf_time(pf_time: u64) {
    unsafe {
        asm!("
            push rbx
            mov rbx, {}
            vmcall
            pop rbx",
            in(reg) pf_time,
            in("eax") HV_PF_TIME,
        );
    }
}

pub fn get_page_table_kbs() -> usize {
    bmk_linux::linux4_4::procfs::meminfo::ProcMeminfo::read()
        .unwrap()
        .page_tables
        .kilobytes()
}

/// Stats from `proc/compact_instrumentation`.
pub struct CompactInstrumentationStats {
    /// Number of operations done (including undos).
    pub ops: usize,

    /// Number of operations undone.
    pub undos: usize,
}

/// Read the contents of `/proc/compact_instrumentation`.
pub fn thp_compact_instrumentation() -> CompactInstrumentationStats {
    const COMPACT_INSTRUMENTATION_PATH: &str = "/proc/compact_instrumentation";

    let stats =
        std::fs::read_to_string(COMPACT_INSTRUMENTATION_PATH).expect("unable to read procfs");

    let mut stats = stats.split_whitespace();

    CompactInstrumentationStats {
        ops: stats.next().unwrap().parse().unwrap(),
        undos: stats.next().unwrap().parse().unwrap(),
    }
}

/// Trigger the given number of compaction attempts.
pub fn trigger_compaction(n: u16) -> Result<(), std::io::Error> {
    const COMPACT_TRIGGER_PATH: &str = "/proc/compact_trigger";

    // Needs to be a C-FFI-compatible string. So we will manually format `n` into a null-terminated
    // ASCII string.
    //
    // We start the least-significant digit and insert a the front...
    let mut s = Vec::with_capacity(6);
    let mut val = n;

    while val > 0 {
        // extract the digit at place
        let digit_at_place: u8 = (val % 10) as u8;

        let c = b'0' + digit_at_place;
        s.insert(0, c); // front = most significant digit

        val /= 10;
    }

    // null terminate
    s.push(0);

    std::fs::write(COMPACT_TRIGGER_PATH, s)
}

/// Pin the calling thread to the given logical core.
///
/// # Panics
///
/// If an error is returned from `sched_setaffinity`.
pub fn set_cpu(core: usize) {
    unsafe {
        let mut cpuset = std::mem::MaybeUninit::<libc::cpu_set_t>::uninit();
        libc::CPU_ZERO(cpuset.assume_init_mut());
        let mut cpuset = cpuset.assume_init();
        libc::CPU_SET(core, &mut cpuset);

        let res = libc::sched_setaffinity(
            /* self */ 0,
            std::mem::size_of::<libc::cpu_set_t>(),
            &cpuset,
        );

        if res != 0 {
            let err = errno::errno();
            panic!("sched_setaffinity failed: {}", err);
        }
    }
}
