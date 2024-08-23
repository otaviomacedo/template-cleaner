#![allow(dead_code)]
use std::{borrow::Borrow, hash, ops::Deref, rc::Rc};

use indexmap::{Equivalent, IndexMap};

/// A bidirectional HashMap
///
/// Uses [IndexMap] so that insertion order is maintained.
///
/// This implementation is largely modeled after the `bimap` crate,
/// but substitutes [IndexMap] for `HashMap`.
#[derive(Debug)]
pub struct BiMap<L, R> {
    ltr: IndexMap<MyRc<L>, MyRc<R>>,
    rtl: IndexMap<MyRc<R>, MyRc<L>>,
}

impl<L, R> Clone for BiMap<L, R>
where
    L: hash::Hash + Eq + PartialEq + Clone,
    R: hash::Hash + Eq + PartialEq + Clone,
{
    fn clone(&self) -> Self {
        Self {
            ltr: self.ltr.clone(),
            rtl: self.rtl.clone(),
        }
    }
}

impl<L, R> BiMap<L, R>
where
    L: hash::Hash + Eq + PartialEq,
    R: hash::Hash + Eq + PartialEq,
{
    /// Creates a new instance of a [BiMap]
    pub fn new() -> Self {
        Self {
            ltr: IndexMap::new(),
            rtl: IndexMap::new(),
        }
    }

    /// Creates a new instance of a [BiMap] with a given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ltr: IndexMap::with_capacity(capacity),
            rtl: IndexMap::with_capacity(capacity),
        }
    }

    /// Looks up the right value given a left value
    pub fn get_by_left<Q>(&self, left: &Q) -> Option<&R>
    where
        L: Borrow<Q>,
        Q: Eq + hash::Hash + ?Sized,
    {
        self.ltr
            .get(TypedReference::convert_ref(left))
            .map(|x| x.deref())
    }

    /// Whether the given left value is contained in the map
    pub fn contains_left<Q>(&self, left: &Q) -> bool
    where
        L: Borrow<Q>,
        Q: Eq + hash::Hash + ?Sized,
    {
        self.ltr.contains_key(TypedReference::convert_ref(left))
    }

    /// Removes the entry given a left value
    pub fn remove_by_left<Q>(&mut self, left: &Q) -> Option<(L, R)>
    where
        L: Borrow<Q>,
        Q: Eq + hash::Hash + ?Sized,
    {
        self.ltr
            .remove(TypedReference::convert_ref(left))
            .map(|right_rc| {
                let left_rc = self.rtl.remove(&right_rc).unwrap();

                // Unwrap the Rcs which should be at refcount == r to get the original values
                (
                    Rc::try_unwrap(left_rc.0).ok().unwrap(),
                    Rc::try_unwrap(right_rc.0).ok().unwrap(),
                )
            })
    }

    /// Looks up a left value given a right value
    pub fn get_by_right<Q>(&self, right: &Q) -> Option<&L>
    where
        R: Borrow<Q>,
        Q: Eq + hash::Hash + ?Sized + Equivalent<R>,
    {
        self.rtl
            .get(TypedReference::convert_ref(right))
            .map(|x| x.deref())
    }

    /// Whether the given right value is contained in the map
    pub fn contains_right<Q>(&self, right: &Q) -> bool
    where
        R: Borrow<Q>,
        Q: Eq + hash::Hash + ?Sized,
    {
        self.rtl.contains_key(TypedReference::convert_ref(right))
    }

    /// Returns an iterator that iterates over all elements
    pub fn iter(&self) -> impl Iterator<Item = (&L, &R)> {
        self.ltr.iter().map(|(l, r)| (l.deref(), r.deref()))
    }

    /// Inserts a new pair of values into both maps
    pub fn insert(&mut self, left: L, right: R) {
        let left = MyRc(Rc::new(left));
        let right = MyRc(Rc::new(right));
        self.ltr.insert(left.clone(), right.clone());
        self.rtl.insert(right, left);
    }

    /// Returns an iterator for the right-hand side values
    pub fn values(&self) -> impl Iterator<Item = &R> {
        self.iter().map(|(_, r)| r)
    }

    pub fn extend(&mut self, other: BiMap<L, R>) {
        self.ltr.extend(other.ltr);
        self.rtl.extend(other.rtl);
    }
}

impl<L, R> Default for BiMap<L, R> {
    fn default() -> Self {
        Self {
            ltr: Default::default(),
            rtl: Default::default(),
        }
    }
}

/// A newtype wrapper around [Rc] to make it possible to implement
/// [Borrow].
///
/// Without a newtype, this would not be possible since neither [Rc]
/// nor [Borrow] are defined in our crate.
///
/// See [TypedReference] to get the value we get when we borrow from
/// [MyRc]`.
#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct MyRc<T>(pub Rc<T>);

impl<T> Clone for MyRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for MyRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// A type that [MyRc] borrows to, so that we can compare it with
/// lookup keys in `get_by_left` and `get_by_right`.
///
/// All of this machinery is necessary to make it possible to use `&str` to look
/// up `String`s, as you would expect to be able to do.
///
/// It works like this:
///
/// - You can `get_by_left()` our map by any type `Q` that `L` borrows to.
/// - We convert the reference of `&Q` to a reference of `&TypedReference<Q>`.
/// - We then `get()` the underlying map with a `&TypedReference<Q>`.
/// - Since `MyRc<L>` borrows to `TypedReference<Q>` (by the definition below), we can
///   use `TypedReference<Q>` to query the map.
///
/// Trick stolen from here: <https://github.com/billyrieger/bimap-rs/blob/3dca651620845a939ee9e5393c0a8fe9fe0a1656/src/mem.rs#L27>
#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
struct TypedReference<T: ?Sized>(pub T);

impl<T: ?Sized> TypedReference<T> {
    /// Converts between reference types
    ///
    /// Turns a `&T` into a &[`TypedReference<T>`]. This is an unsafe cast that doesn't
    /// change the size of the reference, but does change its type.
    pub fn convert_ref(value: &T) -> &Self {
        // safe because Wrapper<T> is #[repr(transparent)]
        // `transmute` will confirm that our types are the same size.
        #[allow(clippy::transmute_ptr_to_ptr)]
        // https://github.com/rust-lang/rust-clippy/issues/6372
        unsafe {
            std::mem::transmute(value)
        }
        // unsafe { &*(value as *const T as *const Self) }
    }
}

impl<K, Q> Borrow<TypedReference<Q>> for MyRc<K>
where
    K: Borrow<Q>,
    Q: ?Sized,
{
    fn borrow(&self) -> &TypedReference<Q> {
        // Rc<K>: Borrow<K>
        let k: &K = self.0.borrow();
        // K: Borrow<Q>
        let q: &Q = k.borrow();

        TypedReference::convert_ref(q)
    }
}

impl<L, R> PartialEq for BiMap<L, R>
where
    L: hash::Hash + Eq + PartialEq,
    R: hash::Hash + Eq + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.ltr == other.ltr
    }
}

impl<L, R> Eq for BiMap<L, R>
where
    L: hash::Hash + Eq + PartialEq,
    R: hash::Hash + Eq + PartialEq,
{
}

impl<L, R> FromIterator<(L, R)> for BiMap<L, R>
where
    L: hash::Hash + Eq + PartialEq,
    R: hash::Hash + Eq + PartialEq,
{
    fn from_iter<I: IntoIterator<Item = (L, R)>>(iterable: I) -> Self {
        let iter = iterable.into_iter();
        let (low, _) = iter.size_hint();
        let mut map = Self::with_capacity(low);
        for (l, r) in iter {
            map.insert(l, r);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::BiMap;

    #[test]
    fn smoketest() {
        let mut x = BiMap::<u64, String>::new();
        x.insert(123, String::from("Hello"));
        assert_eq!(x.get_by_right("Hello"), Some(123_u64).as_ref());
    }
}
