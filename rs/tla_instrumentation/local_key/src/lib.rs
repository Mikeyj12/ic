use pin_project_lite::pin_project;
use std::cell::RefCell;
use std::error::Error;
use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, mem, thread};

/// Declares a new task-local key of type [`tokio::task::LocalKey`].
///
/// # Syntax
///
/// The macro wraps any number of static declarations and makes them local to the current task.
/// Publicity and attributes for each static is preserved. For example:
///
/// # Examples
///
/// ```
/// # use tokio::task_local;
/// task_local! {
///     pub static ONE: u32;
///
///     #[allow(unused)]
///     static TWO: f32;
/// }
/// # fn main() {}
/// ```
///
/// See [`LocalKey` documentation][`tokio::task::LocalKey`] for more
/// information.
///
/// [`tokio::task::LocalKey`]: struct@crate::task::LocalKey
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
macro_rules! task_local {
     // empty (base case for the recursion)
    () => {};

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty; $($rest:tt)*) => {
        $crate::__task_local_inner!($(#[$attr])* $vis $name, $t);
        $crate::task_local!($($rest)*);
    };

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty) => {
        $crate::__task_local_inner!($(#[$attr])* $vis $name, $t);
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __task_local_inner {
    ($(#[$attr:meta])* $vis:vis $name:ident, $t:ty) => {
        $(#[$attr])*
        $vis static $name: $crate::LocalKey<$t> = {
            std::thread_local! {
                static __KEY: std::cell::RefCell<Option<$t>> = const { std::cell::RefCell::new(None) };
            }

            $crate::LocalKey { inner: __KEY }
        };
    };
}

/// A key for task-local data.
///
/// This type is generated by the [`task_local!`] macro.
///
/// Unlike [`std::thread::LocalKey`], `tokio::task::LocalKey` will
/// _not_ lazily initialize the value on first access. Instead, the
/// value is first initialized when the future containing
/// the task-local is first polled by a futures executor, like Tokio.
///
/// # Examples
///
/// ```
/// # async fn dox() {
/// tokio::task_local! {
///     static NUMBER: u32;
/// }
///
/// NUMBER.scope(1, async move {
///     assert_eq!(NUMBER.get(), 1);
/// }).await;
///
/// NUMBER.scope(2, async move {
///     assert_eq!(NUMBER.get(), 2);
///
///     NUMBER.scope(3, async move {
///         assert_eq!(NUMBER.get(), 3);
///     }).await;
/// }).await;
/// # }
/// ```
///
/// [`std::thread::LocalKey`]: struct@std::thread::LocalKey
/// [`task_local!`]: ../macro.task_local.html
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
pub struct LocalKey<T: 'static> {
    #[doc(hidden)]
    pub inner: thread::LocalKey<RefCell<Option<T>>>,
}

impl<T: 'static> LocalKey<T> {
    /// Sets a value `T` as the task-local value for the future `F`.
    ///
    /// On completion of `scope`, the task-local will be dropped.
    ///
    /// ### Panics
    ///
    /// If you poll the returned future inside a call to [`with`] or
    /// [`try_with`] on the same `LocalKey`, then the call to `poll` will panic.
    ///
    /// ### Examples
    ///
    /// ```
    /// # async fn dox() {
    /// tokio::task_local! {
    ///     static NUMBER: u32;
    /// }
    ///
    /// NUMBER.scope(1, async move {
    ///     println!("task local value: {}", NUMBER.get());
    /// }).await;
    /// # }
    /// ```
    ///
    /// [`with`]: fn@Self::with
    /// [`try_with`]: fn@Self::try_with
    pub fn scope<F>(&'static self, value: T, f: F) -> TaskLocalFuture<T, F>
    where
        F: Future,
    {
        TaskLocalFuture {
            local: self,
            slot: Some(value),
            future: Some(f),
            _pinned: PhantomPinned,
        }
    }

    /// Sets a value `T` as the task-local value for the closure `F`.
    ///
    /// On completion of `sync_scope`, the task-local will be dropped.
    ///
    /// ### Panics
    ///
    /// This method panics if called inside a call to [`with`] or [`try_with`]
    /// on the same `LocalKey`.
    ///
    /// ### Examples
    ///
    /// ```
    /// # async fn dox() {
    /// tokio::task_local! {
    ///     static NUMBER: u32;
    /// }
    ///
    /// NUMBER.sync_scope(1, || {
    ///     println!("task local value: {}", NUMBER.get());
    /// });
    /// # }
    /// ```
    ///
    /// [`with`]: fn@Self::with
    /// [`try_with`]: fn@Self::try_with
    #[track_caller]
    pub fn sync_scope<F, R>(&'static self, value: T, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut value = Some(value);
        match self.scope_inner(&mut value, f) {
            Ok(res) => res,
            Err(err) => err.panic(),
        }
    }

    fn scope_inner<F, R>(&'static self, slot: &mut Option<T>, f: F) -> Result<R, ScopeInnerErr>
    where
        F: FnOnce() -> R,
    {
        struct Guard<'a, T: 'static> {
            local: &'static LocalKey<T>,
            slot: &'a mut Option<T>,
        }

        impl<'a, T: 'static> Drop for Guard<'a, T> {
            fn drop(&mut self) {
                // This should not panic.
                //
                // We know that the RefCell was not borrowed before the call to
                // `scope_inner`, so the only way for this to panic is if the
                // closure has created but not destroyed a RefCell guard.
                // However, we never give user-code access to the guards, so
                // there's no way for user-code to forget to destroy a guard.
                //
                // The call to `with` also should not panic, since the
                // thread-local wasn't destroyed when we first called
                // `scope_inner`, and it shouldn't have gotten destroyed since
                // then.
                self.local.inner.with(|inner| {
                    let mut ref_mut = inner.borrow_mut();
                    mem::swap(self.slot, &mut *ref_mut);
                });
            }
        }

        self.inner.try_with(|inner| {
            inner
                .try_borrow_mut()
                .map(|mut ref_mut| mem::swap(slot, &mut *ref_mut))
        })??;

        let guard = Guard { local: self, slot };

        let res = f();

        drop(guard);

        Ok(res)
    }

    /// Accesses the current task-local and runs the provided closure.
    ///
    /// # Panics
    ///
    /// This function will panic if the task local doesn't have a value set.
    #[track_caller]
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        match self.try_with(f) {
            Ok(res) => res,
            Err(_) => panic!("cannot access a task-local storage value without setting it first"),
        }
    }

    /// Accesses the current task-local and runs the provided closure.
    ///
    /// If the task-local with the associated key is not present, this
    /// method will return an `AccessError`. For a panicking variant,
    /// see `with`.
    pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        // If called after the thread-local storing the task-local is destroyed,
        // then we are outside of a closure where the task-local is set.
        //
        // Therefore, it is correct to return an AccessError if `try_with`
        // returns an error.
        let try_with_res = self.inner.try_with(|v| {
            // This call to `borrow` cannot panic because no user-defined code
            // runs while a `borrow_mut` call is active.
            v.borrow().as_ref().map(f)
        });

        match try_with_res {
            Ok(Some(res)) => Ok(res),
            Ok(None) | Err(_) => Err(AccessError { _private: () }),
        }
    }
}

impl<T: Clone + 'static> LocalKey<T> {
    /// Returns a copy of the task-local value
    /// if the task-local value implements `Clone`.
    ///
    /// # Panics
    ///
    /// This function will panic if the task local doesn't have a value set.
    #[track_caller]
    pub fn get(&'static self) -> T {
        self.with(|v| v.clone())
    }
}

impl<T: 'static> fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("LocalKey { .. }")
    }
}

pin_project! {
    /// A future that sets a value `T` of a task local for the future `F` during
    /// its execution.
    ///
    /// The value of the task-local must be `'static` and will be dropped on the
    /// completion of the future.
    ///
    /// Created by the function [`LocalKey::scope`](self::LocalKey::scope).
    ///
    /// ### Examples
    ///
    /// ```
    /// # async fn dox() {
    /// tokio::task_local! {
    ///     static NUMBER: u32;
    /// }
    ///
    /// NUMBER.scope(1, async move {
    ///     println!("task local value: {}", NUMBER.get());
    /// }).await;
    /// # }
    /// ```
    pub struct TaskLocalFuture<T, F>
    where
        T: 'static,
    {
        local: &'static LocalKey<T>,
        slot: Option<T>,
        #[pin]
        future: Option<F>,
        #[pin]
        _pinned: PhantomPinned,
    }

    impl<T: 'static, F> PinnedDrop for TaskLocalFuture<T, F> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if mem::needs_drop::<F>() && this.future.is_some() {
                // Drop the future while the task-local is set, if possible. Otherwise
                // the future is dropped normally when the `Option<F>` field drops.
                let mut future = this.future;
                let _ = this.local.scope_inner(this.slot, || {
                    future.set(None);
                });
            }
        }
    }
}

impl<T, F> TaskLocalFuture<T, F>
where
    T: 'static,
{
    /// Returns the value stored in the task local by this `TaskLocalFuture`.
    ///
    /// The function returns:
    ///
    /// * `Some(T)` if the task local value exists.
    /// * `None` if the task local value has already been taken.
    ///
    /// Note that this function attempts to take the task local value even if
    /// the future has not yet completed. In that case, the value will no longer
    /// be available via the task local after the call to `take_value`.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn dox() {
    /// tokio::task_local! {
    ///     static KEY: u32;
    /// }
    ///
    /// let fut = KEY.scope(42, async {
    ///     // Do some async work
    /// });
    ///
    /// let mut pinned = Box::pin(fut);
    ///
    /// // Complete the TaskLocalFuture
    /// let _ = pinned.as_mut().await;
    ///
    /// // And here, we can take task local value
    /// let value = pinned.as_mut().take_value();
    ///
    /// assert_eq!(value, Some(42));
    /// # }
    /// ```
    pub fn take_value(self: Pin<&mut Self>) -> Option<T> {
        let this = self.project();
        this.slot.take()
    }
}

impl<T: 'static, F: Future> Future for TaskLocalFuture<T, F> {
    type Output = F::Output;

    #[track_caller]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut future_opt = this.future;

        let res = this
            .local
            .scope_inner(this.slot, || match future_opt.as_mut().as_pin_mut() {
                Some(fut) => {
                    let res = fut.poll(cx);
                    if res.is_ready() {
                        future_opt.set(None);
                    }
                    Some(res)
                }
                None => None,
            });

        match res {
            Ok(Some(res)) => res,
            Ok(None) => panic!("`TaskLocalFuture` polled after completion"),
            Err(err) => err.panic(),
        }
    }
}

impl<T: 'static, F> fmt::Debug for TaskLocalFuture<T, F>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// Format the Option without Some.
        struct TransparentOption<'a, T> {
            value: &'a Option<T>,
        }
        impl<'a, T: fmt::Debug> fmt::Debug for TransparentOption<'a, T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.value.as_ref() {
                    Some(value) => value.fmt(f),
                    // Hitting the None branch should not be possible.
                    None => f.pad("<missing>"),
                }
            }
        }

        f.debug_struct("TaskLocalFuture")
            .field("value", &TransparentOption { value: &self.slot })
            .finish()
    }
}

/// An error returned by [`LocalKey::try_with`](method@LocalKey::try_with).
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct AccessError {
    _private: (),
}

impl fmt::Debug for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessError").finish()
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt("task-local value not set", f)
    }
}

impl Error for AccessError {}

enum ScopeInnerErr {
    BorrowError,
    AccessError,
}

impl ScopeInnerErr {
    #[track_caller]
    fn panic(&self) -> ! {
        match self {
            Self::BorrowError => panic!("cannot enter a task-local scope while the task-local storage is borrowed"),
            Self::AccessError => panic!("cannot enter a task-local scope during or after destruction of the underlying thread-local"),
        }
    }
}

impl From<std::cell::BorrowMutError> for ScopeInnerErr {
    fn from(_: std::cell::BorrowMutError) -> Self {
        Self::BorrowError
    }
}

impl From<std::thread::AccessError> for ScopeInnerErr {
    fn from(_: std::thread::AccessError) -> Self {
        Self::AccessError
    }
}
