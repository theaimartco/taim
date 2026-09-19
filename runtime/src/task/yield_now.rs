use std::future::Future;
use std::pin::Pin;
use std::task;

pub(super) async fn yield_now() {
    struct YieldNow {
        yielded: bool,
    }

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<()> {
            if self.yielded {
                return task::Poll::Ready(());
            }

            self.yielded = true;
            cx.waker().wake_by_ref();

            task::Poll::Pending
        }
    }

    YieldNow { yielded: false }.await;
}
