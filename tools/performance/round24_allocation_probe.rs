//! 只在工作区以外的研发进程统计本线程分配。
#[path = "../../crates/cloudleaf-input/examples/research_rank.rs"]
mod rank;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct Allocation {
    bytes: usize,
    calls: usize,
}

thread_local! {
    static ALLOCATION: Cell<Option<Allocation>> = const { Cell::new(None) };
}

struct MeasuredAllocator;

// 例程记录本线程成功申请的字节，系统分配器仍负责申请与释放。
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            allocated(layout.size());
        }
        result
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() {
            allocated(size);
        }
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

fn allocated(bytes: usize) {
    let _ = ALLOCATION.try_with(|slot| {
        if let Some(total) = slot.get() {
            slot.set(Some(Allocation {
                bytes: total.bytes.saturating_add(bytes),
                calls: total.calls.saturating_add(1),
            }));
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rank::run_probe(rank::AllocationProbe {
        start: || ALLOCATION.set(Some(Allocation::default())),
        finish: || {
            let allocation = ALLOCATION.take().unwrap_or_default();
            (allocation.bytes, allocation.calls)
        },
    })
}
