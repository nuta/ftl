use core::ops::Deref;
use core::ops::DerefMut;

use hashbrown::HashMap;
use hashbrown::HashSet;
use rustc_hash::FxBuildHasher;

pub struct FxHashMap<K, V>(HashMap<K, V, FxBuildHasher>);

impl<K, V> Default for FxHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FxHashMap<K, V> {
    pub const fn new() -> Self {
        Self(HashMap::with_hasher(FxBuildHasher))
    }
}

impl<K, V> Deref for FxHashMap<K, V> {
    type Target = HashMap<K, V, FxBuildHasher>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> DerefMut for FxHashMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct FxHashSet<K>(HashSet<K, FxBuildHasher>);

impl<K> Default for FxHashSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> FxHashSet<K> {
    pub const fn new() -> Self {
        Self(HashSet::with_hasher(FxBuildHasher))
    }
}

impl<K> Deref for FxHashSet<K> {
    type Target = HashSet<K, FxBuildHasher>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K> DerefMut for FxHashSet<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
