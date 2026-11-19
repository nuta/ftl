use alloc::boxed::Box;
use alloc::vec::Vec;

enum Node<T> {
    Leaf(Box<[Option<T>]>),
    Inner(Box<[Option<Node<T>>]>),
}

pub struct RadixTree<T, const FANOUT: usize> {
    root: Option<Node<T>>,
    levels: u32,
    capacity: usize,
}

impl<T, const FANOUT: usize> RadixTree<T, FANOUT> {
    pub const fn new(capacity: usize) -> Self {
        // FANOUT == 0: Why do you need this data structure?
        // FANOUT == 1: The levels calculation below never ends.
        const { assert!(FANOUT >= 2) };

        let mut levels = 1;
        while FANOUT.pow(levels) < capacity {
            levels += 1;
        }

        Self {
            root: None,
            levels,
            capacity,
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.capacity {
            return None;
        }

        // Traverse the tree, level by level.
        let mut node = self.root.as_ref()?;
        let mut level = self.levels - 1;
        loop {
            let slot = Self::slot_at(index, level);
            match node {
                Node::Leaf(values) => return values[slot].as_ref(),
                Node::Inner(children) => {
                    node = children[slot].as_ref()?;
                    level -= 1;
                }
            }
        }
    }

    fn new_node(level: u32) -> Option<Node<T>> {
        if level == 0 {
            let slots = Self::new_slots()?;
            Some(Node::Leaf(slots))
        } else {
            let slots = Self::new_slots()?;
            Some(Node::Inner(slots))
        }
    }

    /// Allocates a boxed slice of `Option<T>`, without OOM panic.
    fn new_slots<U>() -> Option<Box<[Option<U>]>> {
        let mut slots = Vec::new();
        if slots.try_reserve_exact(FANOUT).is_err() {
            return None;
        }

        slots.resize_with(FANOUT, || None);
        Some(slots.into_boxed_slice())
    }

    fn slot_at(index: usize, level: u32) -> usize {
        index / FANOUT.pow(level) % FANOUT
    }

    pub fn try_insert(&mut self, index: usize, value: T) -> Result<(), T> {
        if index >= self.capacity {
            return Err(value);
        }

        // Allocate the root node if necessary.
        if self.root.is_none() {
            let Some(root) = Self::new_node(self.levels - 1) else {
                return Err(value);
            };

            self.root = Some(root);
        }

        let Some(mut node) = self.root.as_mut() else {
            return Err(value);
        };

        // Traverse the tree, level by level.
        let mut level = self.levels - 1;
        loop {
            let slot = Self::slot_at(index, level);
            match node {
                Node::Leaf(values) => {
                    // Reached the leaf node. Insert the value.
                    values[slot] = Some(value);
                    return Ok(());
                }
                Node::Inner(children) => {
                    // Allocate a child node if necessary.
                    if children[slot].is_none() {
                        let Some(child) = Self::new_node(level - 1) else {
                            return Err(value);
                        };

                        children[slot] = Some(child);
                    }

                    let Some(child) = children[slot].as_mut() else {
                        // Failed to allocate a child node.
                        return Err(value);
                    };

                    // Traverse to the next level.
                    node = child;
                    level -= 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_level() {
        let mut tree: RadixTree<_, 64> = RadixTree::new(10);
        assert_eq!(tree.get(3), None);
        assert_eq!(tree.try_insert(3, 'A'), Ok(()));
        assert_eq!(tree.get(3), Some(&'A'));
        assert_eq!(tree.get(4), None);
    }

    #[test]
    fn test_multi_level() {
        let mut tree: RadixTree<_, 64> = RadixTree::new(1_000_000);
        assert_eq!(tree.try_insert(0, 0), Ok(()));
        assert_eq!(tree.try_insert(64, 64), Ok(()));
        assert_eq!(tree.try_insert(999_999, 999_999), Ok(()));
        assert_eq!(tree.get(0), Some(&0));
        assert_eq!(tree.get(64), Some(&64));
        assert_eq!(tree.get(999_999), Some(&999_999));
        assert_eq!(tree.get(1), None);
        assert_eq!(tree.get(4096), None);
    }

    #[test]
    fn test_small_fanout() {
        let mut tree: RadixTree<usize, 4> = RadixTree::new(100);
        for index in [0, 3, 4, 63, 64, 99] {
            assert_eq!(tree.try_insert(index, index), Ok(()));
        }

        for index in [0, 3, 4, 63, 64, 99] {
            assert_eq!(tree.get(index), Some(&index));
        }

        for index in [1, 5, 16, 65, 98] {
            assert_eq!(tree.get(index), None);
        }
    }

    #[test]
    fn test_out_of_range() {
        let mut tree: RadixTree<_, 64> = RadixTree::new(100);
        assert_eq!(tree.try_insert(100, 'A'), Err('A'));
        assert_eq!(tree.get(100), None);
    }

    #[test]
    fn test_overwrite() {
        let mut tree: RadixTree<_, 64> = RadixTree::new(100);
        assert_eq!(tree.try_insert(7, 'A'), Ok(()));
        assert_eq!(tree.try_insert(7, 'B'), Ok(()));
        assert_eq!(tree.get(7), Some(&'B'));
    }
}
