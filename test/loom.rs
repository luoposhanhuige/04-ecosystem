use loom::sync::atomic::AtomicUsize;
use loom::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use loom::sync::Arc;
use loom::thread;

#[test]
#[should_panic]
fn buggy_concurrent_inc() {
    loom::model(|| {
        let num = Arc::new(AtomicUsize::new(0));

        let items: Vec<_> = (0..2)
            .map(|_| {
                let num = num.clone();
                thread::spawn(move || {
                    let curr = num.load(Acquire);
                    // This is a bug! this is not atomic! 取了数据然后再存，因为load和store不是原子操作，所以可能会出现竞态条件，导致最终结果不正确。
                    num.store(curr + 1, Release);

                    // fix
                    // num.fetch_add(1, Relaxed);
                })
            })
            .collect();

        for item in items {
            item.join().unwrap();
        }

        assert_eq!(2, num.load(Relaxed));
    });
}
