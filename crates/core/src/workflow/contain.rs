//! Keeps a panic of the CEL interpreter inside the card that caused it.
//!
//! `cel` still has `panic!` sites on evaluation paths. An evaluation runs
//! through [`contain`]: a panic is caught and returned as its message, so the
//! engine freezes that card and the run goes on. The process panic hook is
//! wrapped once, and stays silent only while the current thread is inside
//! [`contain`]; every other panic, on any thread, reaches the previous hook.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::sync::Once;

thread_local! {
    /// Whether this thread is inside [`contain`].
    static CONTAINING: Cell<bool> = const { Cell::new(false) };
}

static HOOK: Once = Once::new();

/// Runs `work`. A panic inside it is caught and returned as its message, and
/// the panic hook prints nothing for it.
///
/// `work` is run as if unwind safe: the caller must not leave state it shares
/// with the rest of the run half-updated when `work` panics. The engine passes
/// an execution that only reads the program and the context.
pub(crate) fn contain<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    HOOK.call_once(install_hook);
    let outer = CONTAINING.replace(true);
    let outcome = panic::catch_unwind(AssertUnwindSafe(work));
    CONTAINING.set(outer);
    outcome.map_err(|payload| message(payload.as_ref()))
}

/// Wraps the current panic hook so it is skipped while this thread contains.
fn install_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
        // During thread teardown the flag may be gone: nothing is contained.
        if !CONTAINING.try_with(Cell::get).unwrap_or(false) {
            previous(info);
        }
    }));
}

/// The text a panic was raised with: `panic!("...")` carries a `&str` or a
/// `String`; anything else has no message to show.
fn message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_owned())
}

#[cfg(test)]
mod tests {
    use std::panic::{self, PanicHookInfo};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, ThreadId};

    use super::{contain, install_hook};

    #[test]
    fn a_value_passes_through() {
        assert_eq!(contain(|| 7), Ok(7));
    }

    #[test]
    fn a_panic_becomes_its_message() {
        assert_eq!(
            contain(|| -> () { panic!("not yet implemented") }),
            Err("not yet implemented".to_owned())
        );
        let value = 3;
        assert_eq!(
            contain(|| -> () { panic!("formatted {value}") }),
            Err("formatted 3".to_owned())
        );
        assert_eq!(
            contain(|| -> () { panic::panic_any(5_u8) }),
            Err("a panic without a message".to_owned())
        );
    }

    #[test]
    fn a_nested_contain_restores_the_outer_scope() {
        let outer = contain(|| {
            let inner = contain(|| -> () { panic!("inner") });
            assert_eq!(inner, Err("inner".to_owned()));
            panic!("outer");
        });
        assert_eq!(outer, Err::<(), _>("outer".to_owned()));
    }

    /// The hook is process-wide: the recorder keeps only this thread's panics,
    /// so a test panicking on another thread does not disturb the count.
    #[test]
    fn the_hook_is_silent_only_inside_contain() {
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let me: ThreadId = thread::current().id();
        let original = panic::take_hook();
        let record = Arc::clone(&seen);
        panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
            if thread::current().id() == me {
                let text = info.payload().downcast_ref::<&str>().copied();
                record
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(text.unwrap_or_default().to_owned());
            }
        }));
        install_hook();

        let contained = contain(|| -> () { panic!("contained") });
        let outside = panic::catch_unwind(|| -> () { panic!("outside") });

        panic::set_hook(original);
        assert_eq!(contained, Err("contained".to_owned()));
        assert!(outside.is_err());
        let seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(seen, ["outside"]);
    }
}
