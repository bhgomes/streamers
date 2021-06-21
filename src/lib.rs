//! Streamers: Stream Utilities

#![cfg_attr(docsrs, feature(doc_cfg), forbid(broken_intra_doc_links))]
#![feature(generic_associated_types)]
#![allow(incomplete_features)]
#![forbid(missing_docs)]
#![no_std]

use {
    core::{
        future::{ready, Future, Ready},
        marker::PhantomData,
        pin::Pin,
        task::{Context, Poll},
    },
    futures::{
        future::Map as MapFuture, stream::ForEach, stream::Map as MapStream, FutureExt, Stream,
        StreamExt as _,
    },
    pin_project_lite::pin_project,
};

/// [`Stream`] Extension Trait
pub trait StreamExt: Stream {
    /// Transforms a stream into a collection.
    #[inline]
    fn collect<B>(self) -> <B as FromStream<Self::Item>>::Future<Self>
    where
        Self: Sized,
        B: FromStream<Self::Item>,
    {
        B::from_stream(self)
    }
}

impl<S> StreamExt for S where S: Stream {}

/// Into Stream Conversion Trait
pub trait IntoStream {
    /// Stream Item
    type Item;

    /// Stream Object
    type IntoStream: Stream<Item = Self::Item>;

    /// Converts `self` into a [`Stream`].
    fn into_stream(self) -> Self::IntoStream;
}

impl<S> IntoStream for S
where
    S: Stream,
{
    type Item = S::Item;

    type IntoStream = S;

    #[inline]
    fn into_stream(self) -> Self::IntoStream {
        self
    }
}

/// From Stream Conversion Trait
pub trait FromStream<T> {
    /// Conversion Future
    type Future<S>: Future<Output = Self>
    where
        S: IntoStream<Item = T>;

    /// Converts a stream into `self`.
    fn from_stream<S>(stream: S) -> Self::Future<S>
    where
        S: IntoStream<Item = T>;
}

/* NOTE: in the future we could have this
/// From Stream Conversion Trait
pub trait FromStream<T> {
    /// Converts a stream into `self`.
    async fn from_stream<S>(stream: S) -> Self
    where
        S: IntoStream<Item = T>;
}
*/

pin_project! {
    /// Future for the [`FromStream`] implementation for `()`
    pub struct UnitFromStreamFuture<S>
    where
        S: IntoStream<Item = ()>,
    {
        #[pin]
        future: UnitFromStreamFutureImpl<S>,
    }
}

type UnitFromStreamFutureImpl<S> =
    ForEach<<S as IntoStream>::IntoStream, Ready<()>, fn(()) -> Ready<()>>;

impl<S> Future for UnitFromStreamFuture<S>
where
    S: IntoStream<Item = ()>,
{
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}

impl FromStream<()> for () {
    type Future<S>
    where
        S: IntoStream<Item = ()>,
    = UnitFromStreamFuture<S>;

    #[inline]
    fn from_stream<S>(stream: S) -> Self::Future<S>
    where
        S: IntoStream<Item = ()>,
    {
        UnitFromStreamFuture {
            future: stream.into_stream().for_each(ready),
        }
    }
}

/// Future for the [`FromStream`] implementation for [`Result<V, E>`]
/// where `V: FromStream<T>`
pub struct ResultFromStreamFuture<T, E, V, S> {
    _error: Result<(), E>,
    _future: (),
    __: PhantomData<(T, V, S)>,
}

impl<T, E, V, S> ResultFromStreamFuture<T, E, V, S> {
    #[inline]
    fn new(stream: S) -> Self
    where
        V: FromStream<T>,
        S: IntoStream<Item = Result<T, E>>,
    {
        /* TODO:
        let mut error = Ok(());
        let future = StreamExt::collect::<V>(ResultShunt {
            stream: stream.into_stream(),
            error: &mut error,
        });
        Self { error, future }
        */
        let _ = stream;
        todo!()
    }
}

impl<T, E, V, S> Future for ResultFromStreamFuture<T, E, V, S>
where
    V: FromStream<T>,
    S: IntoStream<Item = Result<T, E>>,
{
    type Output = Result<V, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // TODO: something like this:
        //
        // ```
        // let this = self.project();
        // let value = futures::ready!(this.future.poll(cx));
        // Poll::Ready(this.error.map(|_| value))
        // ```

        let _ = cx;
        todo!()
    }
}

pin_project! {
    struct ResultShunt<'e, S, E> {
        #[pin]
        stream: S,
        error: &'e mut Result<(), E>,
    }
}

impl<S, T, E> Stream for ResultShunt<'_, S, E>
where
    S: Stream<Item = Result<T, E>>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(next) = futures::ready!(this.stream.poll_next(cx)) {
            match next {
                Ok(t) => return Poll::Ready(Some(t)),
                Err(e) => **this.error = Err(e),
            }
        }
        Poll::Ready(None)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.error.is_err() {
            (0, Some(0))
        } else {
            let (_, upper) = self.stream.size_hint();
            (0, upper)
        }
    }
}

impl<T, E, V> FromStream<Result<T, E>> for Result<V, E>
where
    V: FromStream<T>,
{
    type Future<S>
    where
        S: IntoStream<Item = Result<T, E>>,
    = ResultFromStreamFuture<T, E, V, S>;

    /// Takes each element in the [`Stream`]: if it is an [`Err`], no further
    /// elements are taken, and the [`Err`] is returned. Should no [`Err`] occur, a
    /// container with the values of each [`Result`] is returned.
    ///
    /// [`Err`]: Result::Err
    #[inline]
    fn from_stream<S>(stream: S) -> Self::Future<S>
    where
        S: IntoStream<Item = Result<T, E>>,
    {
        // NOTE: from `impl<T, E, V: FromIterator<T>> FromIterator<Result<T, E>> for Result<V, E>`
        //
        //     FIXME(#11084): This could be replaced with Iterator::scan when this
        //     performance bug is closed.
        //
        ResultFromStreamFuture::new(stream)
    }
}

pin_project! {
    /// Future for the [`FromStream`] implementation for [`Option<V>`]
    /// where `V: FromStream<T>`
    pub struct OptionFromStreamFuture<T, V, S>
    where
        S: IntoStream<Item = Option<T>>,
    {
        #[pin]
        future: OptionFromStreamFutureImpl<T, V, S>,
    }
}

type OptionFromStreamFutureImpl<T, V, S> = MapFuture<
    ResultFromStreamFuture<
        T,
        (),
        V,
        MapStream<<S as IntoStream>::IntoStream, fn(Option<T>) -> Result<T, ()>>,
    >,
    fn(Result<V, ()>) -> Option<V>,
>;

impl<T, V, S> Future for OptionFromStreamFuture<T, V, S>
where
    V: FromStream<T>,
    S: IntoStream<Item = Option<T>>,
{
    type Output = Option<V>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}

impl<T, V> FromStream<Option<T>> for Option<V>
where
    V: FromStream<T>,
{
    type Future<S>
    where
        S: IntoStream<Item = Option<T>>,
    = OptionFromStreamFuture<T, V, S>;

    /// Takes each element in the [`Stream`]: if it is [`None`], no further
    /// elements are taken, and the [`None`] is returned. Should no [`None`] occur, a
    /// container with the values of each [`Option`] is returned.
    ///
    /// [`None`]: Option::None
    #[inline]
    fn from_stream<S>(stream: S) -> Self::Future<S>
    where
        S: IntoStream<Item = Option<T>>,
    {
        // NOTE: from `impl<T, V: FromIterator<T>> FromIterator<Option<T>> for Option<V>`
        //
        //     FIXME(#11084): This could be replaced with Iterator::scan when this
        //     performance bug is closed.
        //

        #[inline]
        fn ok_or_unit<T>(x: Option<T>) -> Result<T, ()> {
            x.ok_or(())
        }

        OptionFromStreamFuture {
            future: StreamExt::collect::<Result<_, _>>(
                stream
                    .into_stream()
                    .map(ok_or_unit as fn(Option<T>) -> Result<T, ()>),
            )
            .map(Result::ok),
        }
    }
}
