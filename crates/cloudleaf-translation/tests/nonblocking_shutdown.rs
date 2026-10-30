use cloudleaf_translation::{ProviderRegistry, ServiceLimits, TranslationService};
use std::time::{Duration, Instant};

#[test]
fn closing_idle_workers_is_nonblocking_and_eventually_reclaims_them() {
    let mut service =
        TranslationService::new(ProviderRegistry::new(), ServiceLimits::default()).unwrap();
    service.begin_shutdown();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !service.workers_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    service.shutdown().unwrap();
}
