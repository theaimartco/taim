use crate::futures::futures::stream;

use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) enum InternalHandle {
    Manual(stream::AbortHandle),
    AbortOnDrop(Option<Arc<stream::AbortHandle>>),
}

impl InternalHandle {
    fn as_ref(&self) -> &stream::AbortHandle {
        match self {
            Self::Manual(handle) => handle,
            Self::AbortOnDrop(Some(handle)) => handle.as_ref(),
            Self::AbortOnDrop(None) => unreachable!("a live handle always owns an abort token"),
        }
    }
}

/// A handle to a [`super::Task`] that can be used for aborting it.
#[derive(Debug, Clone)]
pub struct Handle {
    pub(super) internal: InternalHandle,
}

impl Handle {
    /// Aborts the task of this [`Handle`].
    pub fn abort(&self) {
        self.internal.as_ref().abort();
    }

    /// Returns a handle that aborts its task when its last clone is dropped.
    pub fn abort_on_drop(self) -> Self {
        match &self.internal {
            InternalHandle::Manual(handle) => Self {
                internal: InternalHandle::AbortOnDrop(Some(Arc::new(handle.clone()))),
            },
            InternalHandle::AbortOnDrop(_) => self,
        }
    }

    /// Returns `true` if the task of this [`Handle`] has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.internal.as_ref().is_aborted()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let InternalHandle::AbortOnDrop(handle) = &mut self.internal else {
            return;
        };

        if let Some(handle) = handle.take().and_then(Arc::into_inner) {
            handle.abort();
        }
    }
}
