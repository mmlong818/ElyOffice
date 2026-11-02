//! Test cleanup at thread end; gpui test apps never drop.

use std::cell::RefCell;

/// The closures this thread runs when it ends.
struct Endings(Vec<Box<dyn FnOnce()>>);

impl Drop for Endings {
    fn drop(&mut self) {
        for end in self.0.drain(..) {
            end();
        }
    }
}

thread_local! {
    static ENDINGS: RefCell<Endings> = const { RefCell::new(Endings(Vec::new())) };
}

/// Runs `end` when the calling thread ends.
pub fn at_thread_end(end: impl FnOnce() + 'static) {
    ENDINGS.with(|endings| endings.borrow_mut().0.push(Box::new(end)));
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    #[test]
    fn endings_run_when_the_thread_ends() {
        let ran = Arc::new(Mutex::new(Vec::new()));
        let seen = ran.clone();
        std::thread::spawn(move || {
            for each in 1..=2 {
                let seen = seen.clone();
                super::at_thread_end(move || seen.lock().unwrap().push(each));
            }
            assert!(seen.lock().unwrap().is_empty());
        })
        .join()
        .unwrap();
        assert_eq!(*ran.lock().unwrap(), [1, 2]);
    }
}
