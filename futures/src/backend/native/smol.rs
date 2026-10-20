//! A `smol` backend.

/// A `smol` executor.
#[derive(Debug)]
pub struct Executor;

impl crate::Executor for Executor {
    fn new() -> Result<Self, futures::io::Error> {
        Ok(Self)
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        smol::spawn(future).detach();
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        smol::block_on(future)
    }
}

pub mod time {
    //! Listen and react to time.
    use crate::MaybeSend;
    use crate::core::time::{Duration, Instant};
    use crate::subscription::Subscription;

    const MIN_INTERVAL: Duration = Duration::from_millis(1);

    /// Keep zero-period timers from panicking or spinning the UI event loop.
    #[inline]
    pub(super) fn safe_interval(value: Duration) -> Duration {
        value.max(MIN_INTERVAL)
    }

    use futures::stream;

    /// Returns a [`Subscription`] that produces messages at a set interval.
    ///
    /// The first message is produced after a `duration`, and then continues to
    /// produce more messages every `duration` after that.
    pub fn every(duration: Duration) -> Subscription<Instant> {
        Subscription::run_with(duration, |duration| {
            use futures::stream::StreamExt;

            let interval = safe_interval(*duration);
            let start = Instant::now() + interval;

            smol::Timer::interval_at(start, interval).boxed()
        })
    }

    /// Returns a [`Subscription`] that runs the given async function at a
    /// set interval; producing the result of the function as output.
    pub fn repeat<F, T>(f: fn() -> F, interval: Duration) -> Subscription<T>
    where
        F: Future<Output = T> + MaybeSend + 'static,
        T: MaybeSend + 'static,
    {
        Subscription::run_with((f, interval), |(f, interval)| {
            let f = *f;
            let interval = safe_interval(*interval);

            stream::unfold(0, move |i| async move {
                if i > 0 {
                    _ = smol::Timer::after(interval).await;
                }

                Some((f().await, i + 1))
            })
        })
    }
}

#[cfg(test)]
mod taim_timer_contract_tests {
    use super::time::safe_interval;
    use crate::core::time::Duration;

    #[test]
    fn smol_interval_never_runs_at_zero_period() {
        assert_eq!(safe_interval(Duration::ZERO), Duration::from_millis(1));
        assert_eq!(
            safe_interval(Duration::from_millis(25)),
            Duration::from_millis(25)
        );
    }
}
