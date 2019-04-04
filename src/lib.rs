//! Some utilities for experiments. These are mostly wrappers around libc.

#![feature(asm)]

/// The host elapsed time hypercall number.
const HV_GET_HOST_ELAPSED: u64 = 0x9;

/// The host nop hypercall number.
const HV_NOP: u64 = 0xA;

/// The host elapsed time calibration hypercall number.
const HV_CALIBRATE: u64 = 0xB;

/// The host elapsed time calibration hypercall number.
const HV_PF_TIME: u64 = 0xC;

/// Run the `vmcall 0x0009` instruction and return the value
#[inline(always)]
pub fn vmcall_host_elapsed() -> u64 {
    let hi: u32;
    let lo: u32;

    unsafe {
        asm!("
		mov $$0, %edx
		vmcall"
		: "={eax}"(lo), "={edx}"(hi)
		: "{eax}"(HV_GET_HOST_ELAPSED)
		:
		: "volatile");
    }

    lo as u64 | ((hi as u64) << 32)
}

/// Run the `vmcall 0x000A` instruction
#[inline(always)]
pub fn vmcall_nop() {
    unsafe {
        asm!("
		vmcall"
		:
		: "{eax}"(HV_NOP)
		:
		: "volatile");
    }
}

/// Run the `vmcall 0x000B` instruction and with the given value
#[inline(always)]
pub fn vmcall_calibrate(too_low: bool) {
    unsafe {
        asm!("
		vmcall"
		:
		: "{eax}"(HV_CALIBRATE), "{rbx}"(if too_low { 1 } else { 0 })
		:
		: "volatile");
    }
}

/// Run the `vmcall 0x000C` instruction
#[inline(always)]
pub fn vmcall_pf_time(pf_time: u64) {
    unsafe {
        asm!("
		vmcall"
		:
		: "{eax}"(HV_PF_TIME), "{rbx}"(pf_time)
		:
		: "volatile");
    }
}

pub fn get_page_table_kbs() -> usize {
    bmk_linux::linux4_4::procfs::meminfo::ProcMeminfo::read()
        .unwrap()
        .page_tables
        .kilobytes()
}

pub enum THPCompactionSyscallWhich {
    Ops,
    UndoneOps,
}

pub const THP_COMPACTION_SYSCALL_NR: libc::c_long = 335;

/// Call syscall 335 to get the number of THP compaction operations that were done and undone.
pub fn thp_compaction_syscall(which: THPCompactionSyscallWhich) -> isize {
    unsafe {
        libc::syscall(
            THP_COMPACTION_SYSCALL_NR,
            match which {
                THPCompactionSyscallWhich::Ops => 0,
                THPCompactionSyscallWhich::UndoneOps => 1,
            },
        ) as isize
    }
}
