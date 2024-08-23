use crate::bimap::BiMap;
use itertools::Itertools;
use std::hash::Hash;

#[derive(Debug)]
pub struct UnionFind<T> {
    array: Vec<usize>,
    map: BiMap<T, usize>,
}

impl<T: Eq + Hash + Clone> UnionFind<T> {
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> UnionFind<T> {
        let mut array: Vec<usize> = vec![];
        let mut map: BiMap<T, usize> = BiMap::new();

        for t in iter {
            let i = array.len();
            array.push(i);
            map.insert(t, i);
        }

        UnionFind { array, map }
    }

    pub fn union(&mut self, t1: T, t2: T) -> Result<(), UnionError> {
        let r1 = self.find_to_result(&t1)?;
        let r2 = self.find_to_result(&t2)?;

        let child_idx = self.map.get_by_left(&r1).unwrap();
        let parent_idx = self.map.get_by_left(&r2).unwrap();

        self.array[*child_idx] = *parent_idx;

        Ok(())
    }

    pub fn find(&mut self, t: &T) -> Option<T> {
        let mut idx = self.map.get_by_left(t)?.clone();
        loop {
            let is_root = self.array[idx] == idx;
            if is_root {
                return self.map.get_by_right(&idx).cloned();
            } else {
                // Search with path compression
                let tmp_idx = self.array[idx];
                self.array[idx] = self.array[tmp_idx];
                idx = tmp_idx;
            }
        }
    }

    fn find_to_result(&mut self, t: &T) -> Result<T, UnionError> {
        self.find(t)
            .map(Ok)
            .unwrap_or(Err(UnionError::ElementNotFound))
    }
}

#[derive(Debug)]
pub enum UnionError {
    ElementNotFound,
}

#[cfg(test)]
mod tests {
    use crate::union_find::{UnionError, UnionFind};

    #[test]
    fn union_find() -> Result<(), UnionError> {
        let mut uf = UnionFind::from_iter(["A", "B", "C", "D", "E", "F", "G", "H"]);
        uf.union(&"A", &"B")?;
        uf.union(&"C", &"D")?;
        uf.union(&"A", &"C")?;

        assert_eq!(uf.find(&"A"), Some("D"));
        assert_eq!(uf.find(&"E"), Some("E"));

        Ok(())
    }

    #[test]
    fn element_not_found() -> Result<(), UnionError> {
        let mut uf = UnionFind::from_iter(["A", "B", "C", "D", "E", "F", "G", "H"]);
        assert_eq!(uf.find(&"Z"), None);

        Ok(())
    }
}
