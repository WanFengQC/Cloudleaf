//! 模块级恢复容量；接管前原子预留，材料转交不会释放后重新争抢。
use super::service::Lease;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Usage {
    pub hosts: usize,
    pub batches: usize,
    pub text_bytes: usize,
    pub fixed_bytes: usize,
}
const LIMITS: Usage = Usage {
    hosts: 64,
    batches: 64,
    text_bytes: 64 * 1024 * 1024,
    fixed_bytes: 128 * 1024 * 1024,
};

struct Pool {
    limits: Usage,
    hosts: AtomicUsize,
    batches: AtomicUsize,
    text_bytes: AtomicUsize,
    fixed_bytes: AtomicUsize,
}

/// 不可复制的计费凭证；外部 Arc 共享凭证而不是重复计费。
pub(crate) struct Reservation {
    pool: Arc<Pool>,
    charge: Usage,
    _lease: Lease,
}
impl Reservation {
    /// 调用方核对实际保全需求；只读成本不能用于转移或清空活跃额度。
    pub fn charge(&self) -> Usage {
        self.charge
    }
}
static GLOBAL: OnceLock<Arc<Pool>> = OnceLock::new();

pub(crate) fn reservation_overhead() -> usize {
    // 两个引用计数、凭证自身及保守分配元数据；业务对象由调用方另计。
    std::mem::size_of::<Reservation>() + 2 * std::mem::size_of::<usize>() + 32
}
pub(crate) fn limits() -> Usage {
    LIMITS
}
fn global() -> &'static Arc<Pool> {
    GLOBAL.get_or_init(|| Arc::new(Pool::new(limits())))
}
pub(crate) fn usage() -> Usage {
    global().usage()
}
fn host_charge(fixed: usize) -> Option<Usage> {
    if fixed == 0 {
        return None;
    }
    Some(Usage {
        hosts: 1,
        fixed_bytes: fixed.checked_add(reservation_overhead())?,
        ..Usage::default()
    })
}
fn batch_charge(text: usize, fixed: usize) -> Option<Usage> {
    if text == 0 || fixed == 0 {
        return None;
    }
    Some(Usage {
        batches: 1,
        text_bytes: text,
        fixed_bytes: fixed.checked_add(reservation_overhead())?,
        ..Usage::default()
    })
}
pub(crate) fn reserve_host(fixed_bytes: usize) -> Option<Reservation> {
    global().reserve(host_charge(fixed_bytes)?)
}
pub(crate) fn reserve_batch(text_capacity: usize, fixed_capacity: usize) -> Option<Reservation> {
    global().reserve(batch_charge(text_capacity, fixed_capacity)?)
}
pub(crate) fn can_reserve_host(fixed_bytes: usize) -> bool {
    host_charge(fixed_bytes).is_some_and(|charge| global().can_reserve(charge))
}
pub(crate) fn can_reserve_batch(text_capacity: usize, fixed_capacity: usize) -> bool {
    batch_charge(text_capacity, fixed_capacity).is_some_and(|charge| global().can_reserve(charge))
}

impl Pool {
    fn new(limits: Usage) -> Self {
        Self {
            limits,
            hosts: AtomicUsize::new(0),
            batches: AtomicUsize::new(0),
            text_bytes: AtomicUsize::new(0),
            fixed_bytes: AtomicUsize::new(0),
        }
    }
    fn usage(&self) -> Usage {
        // 多个计数只作诊断采样；真正取得容量必须经 reserve 的每维 CAS。
        Usage {
            hosts: self.hosts.load(Ordering::Acquire),
            batches: self.batches.load(Ordering::Acquire),
            text_bytes: self.text_bytes.load(Ordering::Acquire),
            fixed_bytes: self.fixed_bytes.load(Ordering::Acquire),
        }
    }
    fn can_reserve(&self, charge: Usage) -> bool {
        let used = self.usage();
        [
            (used.hosts, charge.hosts, self.limits.hosts),
            (used.batches, charge.batches, self.limits.batches),
            (used.text_bytes, charge.text_bytes, self.limits.text_bytes),
            (
                used.fixed_bytes,
                charge.fixed_bytes,
                self.limits.fixed_bytes,
            ),
        ]
        .iter()
        .all(|(used, amount, limit)| used.checked_add(*amount).is_some_and(|sum| sum <= *limit))
    }
    fn reserve(self: &Arc<Self>, charge: Usage) -> Option<Reservation> {
        let dimensions = [
            (&self.hosts, charge.hosts, self.limits.hosts),
            (&self.batches, charge.batches, self.limits.batches),
            (&self.text_bytes, charge.text_bytes, self.limits.text_bytes),
            (
                &self.fixed_bytes,
                charge.fixed_bytes,
                self.limits.fixed_bytes,
            ),
        ];
        for (index, (counter, amount, limit)) in dimensions.iter().enumerate() {
            if counter
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(*amount).filter(|sum| sum <= limit)
                })
                .is_err()
            {
                // 只回滚本次已经取得的维度，不影响现有材料和其他线程预留。
                for (acquired, amount, _) in dimensions[..index].iter().rev() {
                    acquired.fetch_sub(*amount, Ordering::AcqRel);
                }
                return None;
            }
        }
        Some(Reservation {
            pool: self.clone(),
            charge,
            _lease: Lease::new(),
        })
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.pool
            .fixed_bytes
            .fetch_sub(self.charge.fixed_bytes, Ordering::AcqRel);
        self.pool
            .text_bytes
            .fetch_sub(self.charge.text_bytes, Ordering::AcqRel);
        self.pool
            .batches
            .fetch_sub(self.charge.batches, Ordering::AcqRel);
        self.pool
            .hosts
            .fetch_sub(self.charge.hosts, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Barrier, thread, time::Instant};

    fn pool() -> Arc<Pool> {
        Arc::new(Pool::new(Usage {
            hosts: 2,
            batches: 3,
            text_bytes: 100,
            fixed_bytes: 1000,
        }))
    }
    #[test]
    fn every_limit_rejects_without_changing_existing_reservations() {
        for charge in [
            Usage {
                hosts: 3,
                ..Usage::default()
            },
            Usage {
                batches: 4,
                ..Usage::default()
            },
            Usage {
                text_bytes: 101,
                ..Usage::default()
            },
            Usage {
                fixed_bytes: 1001,
                ..Usage::default()
            },
        ] {
            let pool = pool();
            let retained = pool
                .reserve(Usage {
                    batches: 1,
                    text_bytes: 1,
                    fixed_bytes: 1,
                    ..Usage::default()
                })
                .unwrap();
            let before = pool.usage();
            assert!(!pool.can_reserve(charge));
            assert!(pool.reserve(charge).is_none());
            assert_eq!(pool.usage(), before);
            drop(retained);
            assert_eq!(pool.usage(), Usage::default());
        }
    }
    #[test]
    fn overflow_and_zero_cost_never_create_a_global_token() {
        assert!(host_charge(0).is_none());
        assert!(host_charge(usize::MAX).is_none());
        assert!(batch_charge(0, 1).is_none());
        assert!(batch_charge(1, 0).is_none());
        assert!(batch_charge(1, usize::MAX).is_none());
        let pool = pool();
        let token = pool
            .reserve(Usage {
                fixed_bytes: 1,
                ..Usage::default()
            })
            .unwrap();
        assert!(
            pool.reserve(Usage {
                fixed_bytes: usize::MAX,
                ..Usage::default()
            })
            .is_none()
        );
        assert_eq!(pool.usage().fixed_bytes, 1);
        drop(token);
        assert_eq!(pool.usage(), Usage::default());
    }
    #[test]
    fn shared_token_preserves_cost_until_last_owner_explicitly_releases_it() {
        let pool = pool();
        let charge = Usage {
            batches: 1,
            text_bytes: 99,
            fixed_bytes: 300,
            ..Usage::default()
        };
        let ledger = Arc::new(pool.reserve(charge).unwrap());
        assert_eq!(ledger.charge(), charge);
        let material_store = ledger.clone();
        drop(ledger);
        assert_eq!(pool.usage(), charge);
        assert!(pool.reserve(charge).is_none());
        drop(material_store);
        assert_eq!(pool.usage(), Usage::default());
        assert!(pool.reserve(charge).is_some());
    }
    #[test]
    fn read_only_hint_is_not_a_reservation_or_a_guarantee() {
        let pool = pool();
        let charge = Usage {
            hosts: 2,
            ..Usage::default()
        };
        assert!(pool.can_reserve(charge));
        assert_eq!(pool.usage(), Usage::default());
        let token = pool.reserve(charge).unwrap();
        assert!(!pool.can_reserve(charge));
        assert!(pool.reserve(charge).is_none());
        drop(token);
        assert!(pool.can_reserve(charge));
    }
    #[test]
    fn concurrent_acquisition_never_exceeds_any_dimension_and_releases_all_tokens() {
        let pool = Arc::new(Pool::new(Usage {
            hosts: 8,
            batches: 8,
            text_bytes: 800,
            fixed_bytes: 8000,
        }));
        let start = Arc::new(Barrier::new(16));
        let workers: Vec<_> = (0..16)
            .map(|_| {
                let pool = pool.clone();
                let start = start.clone();
                thread::spawn(move || {
                    start.wait();
                    for _ in 0..2000 {
                        let held = pool.reserve(Usage {
                            hosts: 1,
                            batches: 1,
                            text_bytes: 100,
                            fixed_bytes: 1000,
                        });
                        let observed = pool.usage();
                        assert!(observed.hosts <= 8 && observed.batches <= 8);
                        assert!(observed.text_bytes <= 800 && observed.fixed_bytes <= 8000);
                        thread::yield_now();
                        drop(held);
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(pool.usage(), Usage::default());
    }
    #[test]
    fn epoch_capacity_pressure_records_real_token_size_and_fails_closed() {
        let pool = Arc::new(Pool::new(limits()));
        let batch = batch_charge(64 * 1024, 192 * 1024).unwrap();
        let started = Instant::now();
        let mut peak = 0;
        for _ in 0..100 {
            let mut held = Vec::new();
            while let Some(token) = pool.reserve(batch) {
                held.push(token);
            }
            assert_eq!(held.len(), 64);
            peak = held.len();
            let full = pool.usage();
            assert_eq!(full.batches, 64);
            assert_eq!(full.text_bytes, full.batches * batch.text_bytes);
            assert!(full.text_bytes <= limits().text_bytes);
            assert!(full.fixed_bytes <= limits().fixed_bytes);
            assert!(pool.reserve(batch).is_none());
            drop(held);
            assert_eq!(pool.usage(), Usage::default());
        }
        println!(
            "{{\"scope\":\"budget_counter_pressure_only\",\"cycles\":100,\"batch_peak\":{peak},\"token_size\":{},\"charged_token_overhead\":{},\"elapsed_ns\":{},\"production_object_memory_proven\":false}}",
            std::mem::size_of::<Reservation>(),
            reservation_overhead(),
            started.elapsed().as_nanos()
        );
    }
}
