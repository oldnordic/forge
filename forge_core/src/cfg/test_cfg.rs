use crate::types::BlockId;
use std::collections::{HashMap, HashSet, VecDeque};

use super::types::{DominatorTree, Loop, Path};

#[derive(Clone, Debug)]
pub struct TestCfg {
    pub entry: BlockId,
    pub exits: HashSet<BlockId>,
    pub error_blocks: HashSet<BlockId>,
    pub successors: HashMap<BlockId, Vec<BlockId>>,
    pub predecessors: HashMap<BlockId, Vec<BlockId>>,
}

impl TestCfg {
    pub fn new(entry: BlockId) -> Self {
        Self {
            entry,
            exits: HashSet::new(),
            error_blocks: HashSet::new(),
            successors: HashMap::new(),
            predecessors: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, from: BlockId, to: BlockId) -> &mut Self {
        self.successors.entry(from).or_default().push(to);
        self.predecessors.entry(to).or_default().push(from);
        self
    }

    pub fn add_exit(&mut self, block: BlockId) -> &mut Self {
        self.exits.insert(block);
        self
    }

    pub fn add_error(&mut self, block: BlockId) -> &mut Self {
        self.error_blocks.insert(block);
        self
    }

    pub fn chain(start: i64, count: usize) -> Self {
        let mut cfg = Self::new(BlockId(start));
        for i in start..(start + count as i64 - 1) {
            cfg.add_edge(BlockId(i), BlockId(i + 1));
        }
        cfg.add_exit(BlockId(start + count as i64 - 1));
        cfg
    }

    pub fn if_else() -> Self {
        let mut cfg = Self::new(BlockId(0));
        cfg.add_edge(BlockId(0), BlockId(1))
            .add_edge(BlockId(0), BlockId(2))
            .add_edge(BlockId(1), BlockId(3))
            .add_edge(BlockId(2), BlockId(3))
            .add_exit(BlockId(3));
        cfg
    }

    pub fn simple_loop() -> Self {
        let mut cfg = Self::new(BlockId(0));
        cfg.add_edge(BlockId(0), BlockId(1))
            .add_edge(BlockId(1), BlockId(2))
            .add_edge(BlockId(2), BlockId(1))
            .add_edge(BlockId(1), BlockId(3))
            .add_exit(BlockId(3));
        cfg
    }

    pub fn enumerate_paths(&self) -> Vec<Path> {
        let mut paths = Vec::new();
        let mut current = vec![self.entry];
        let mut visited = HashSet::new();
        self.dfs(&mut paths, &mut current, &mut visited, self.entry);
        paths
    }

    fn dfs(
        &self,
        paths: &mut Vec<Path>,
        current: &mut Vec<BlockId>,
        visited: &mut HashSet<BlockId>,
        block: BlockId,
    ) {
        if self.exits.contains(&block) {
            paths.push(Path::new(current.clone()));
            return;
        }
        if visited.contains(&block) {
            return;
        }
        visited.insert(block);
        if let Some(successors) = self.successors.get(&block) {
            for &succ in successors {
                current.push(succ);
                self.dfs(paths, current, visited, succ);
                current.pop();
            }
        }
        visited.remove(&block);
    }

    pub fn compute_dominators(&self) -> DominatorTree {
        let mut blocks: HashSet<BlockId> = HashSet::new();
        blocks.insert(self.entry);
        for (from, tos) in &self.successors {
            blocks.insert(*from);
            for to in tos {
                blocks.insert(*to);
            }
        }

        if blocks.is_empty() {
            return DominatorTree::new(self.entry);
        }

        let block_list: Vec<BlockId> = blocks.iter().copied().collect();
        let mut dom: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();

        for &block in &block_list {
            if block == self.entry {
                dom.insert(block, HashSet::from([self.entry]));
            } else {
                dom.insert(block, blocks.clone());
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for &block in &block_list {
                if block == self.entry {
                    continue;
                }
                let preds = self.predecessors.get(&block);
                if preds.is_none() || preds.unwrap().is_empty() {
                    continue;
                }
                let mut new_dom: HashSet<BlockId> =
                    dom.get(&preds.unwrap()[0]).cloned().unwrap_or_default();
                for pred in &preds.unwrap()[1..] {
                    if let Some(pred_dom) = dom.get(pred) {
                        new_dom = new_dom.intersection(pred_dom).copied().collect();
                    }
                }
                new_dom.insert(block);
                if dom.get(&block) != Some(&new_dom) {
                    dom.insert(block, new_dom);
                    changed = true;
                }
            }
        }

        let mut idom: HashMap<BlockId, BlockId> = HashMap::new();
        for &block in &block_list {
            if block == self.entry {
                continue;
            }
            if let Some(doms) = dom.get(&block) {
                let mut best_candidate: Option<BlockId> = None;
                let mut best_size = 0;

                for &candidate in doms {
                    if candidate == block {
                        continue;
                    }
                    if let Some(candidate_doms) = dom.get(&candidate) {
                        if candidate_doms.len() > best_size {
                            best_size = candidate_doms.len();
                            best_candidate = Some(candidate);
                        }
                    }
                }

                if let Some(candidate) = best_candidate {
                    idom.insert(block, candidate);
                }
            }
        }

        DominatorTree {
            root: self.entry,
            dominators: idom,
        }
    }

    pub fn detect_loops(&self) -> Vec<Loop> {
        let dom = self.compute_dominators();
        let mut loops = Vec::new();

        for (from, tos) in &self.successors {
            for to in tos {
                if dom.dominates(*to, *from) {
                    let header = *to;
                    let mut loop_blocks = HashSet::new();
                    loop_blocks.insert(header);
                    let mut worklist = VecDeque::new();
                    worklist.push_back(*from);

                    while let Some(block) = worklist.pop_front() {
                        if loop_blocks.contains(&block) {
                            continue;
                        }
                        if dom.dominates(header, block) || block == header {
                            loop_blocks.insert(block);
                            if let Some(preds) = self.predecessors.get(&block) {
                                for &pred in preds {
                                    if !loop_blocks.contains(&pred) {
                                        worklist.push_back(pred);
                                    }
                                }
                            }
                        }
                    }

                    let mut blocks: Vec<BlockId> =
                        loop_blocks.into_iter().filter(|&b| b != header).collect();
                    blocks.sort();
                    loops.push(Loop::with_depth(header, blocks, 0));
                }
            }
        }

        loops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_cfg_chain() {
        let cfg = TestCfg::chain(0, 5);

        assert_eq!(cfg.entry, BlockId(0));
        assert!(cfg.exits.contains(&BlockId(4)));

        assert_eq!(cfg.successors.get(&BlockId(0)), Some(&vec![BlockId(1)]));
        assert_eq!(cfg.successors.get(&BlockId(1)), Some(&vec![BlockId(2)]));
        assert_eq!(cfg.successors.get(&BlockId(2)), Some(&vec![BlockId(3)]));
        assert_eq!(cfg.successors.get(&BlockId(3)), Some(&vec![BlockId(4)]));
    }

    #[test]
    fn test_test_cfg_if_else() {
        let cfg = TestCfg::if_else();

        assert_eq!(cfg.entry, BlockId(0));
        assert!(cfg.exits.contains(&BlockId(3)));

        let succ0 = cfg.successors.get(&BlockId(0)).unwrap();
        assert!(succ0.contains(&BlockId(1)));
        assert!(succ0.contains(&BlockId(2)));
        assert_eq!(cfg.successors.get(&BlockId(1)), Some(&vec![BlockId(3)]));
        assert_eq!(cfg.successors.get(&BlockId(2)), Some(&vec![BlockId(3)]));
    }

    #[test]
    fn test_paths_simple_chain() {
        let cfg = TestCfg::chain(0, 4);
        let paths = cfg.enumerate_paths();

        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].blocks,
            vec![BlockId(0), BlockId(1), BlockId(2), BlockId(3)]
        );
        assert!(paths[0].is_normal());
    }

    #[test]
    fn test_paths_if_else() {
        let cfg = TestCfg::if_else();
        let paths = cfg.enumerate_paths();

        assert_eq!(paths.len(), 2);

        assert_eq!(paths[0].entry(), Some(BlockId(0)));
        assert_eq!(paths[0].exit(), Some(BlockId(3)));
        assert_eq!(paths[1].entry(), Some(BlockId(0)));
        assert_eq!(paths[1].exit(), Some(BlockId(3)));

        let paths_set: HashSet<_> = paths.iter().map(|p| p.blocks.clone()).collect();

        assert!(paths_set.contains(&vec![BlockId(0), BlockId(1), BlockId(3)]));
        assert!(paths_set.contains(&vec![BlockId(0), BlockId(2), BlockId(3)]));
    }

    #[test]
    fn test_dominators_chain() {
        let cfg = TestCfg::chain(0, 5);
        let dom = cfg.compute_dominators();

        assert_eq!(dom.root, BlockId(0));
        assert_eq!(dom.immediate_dominator(BlockId(1)), Some(BlockId(0)));
        assert_eq!(dom.immediate_dominator(BlockId(2)), Some(BlockId(1)));
        assert_eq!(dom.immediate_dominator(BlockId(3)), Some(BlockId(2)));
        assert_eq!(dom.immediate_dominator(BlockId(4)), Some(BlockId(3)));
    }

    #[test]
    fn test_dominators_if_else() {
        let cfg = TestCfg::if_else();
        let dom = cfg.compute_dominators();

        assert!(dom.dominates(BlockId(0), BlockId(0)));
        assert!(dom.dominates(BlockId(0), BlockId(1)));
        assert!(dom.dominates(BlockId(0), BlockId(2)));
        assert!(dom.dominates(BlockId(0), BlockId(3)));
        assert_eq!(dom.immediate_dominator(BlockId(3)), Some(BlockId(0)));
    }

    #[test]
    fn test_loops_simple_loop() {
        let cfg = TestCfg::simple_loop();
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].header, BlockId(1));
        assert!(!loops[0].blocks.is_empty());
        assert!(loops[0].blocks.contains(&BlockId(2)));
    }

    #[test]
    fn test_loops_none_in_chain() {
        let cfg = TestCfg::chain(0, 5);
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 0);
    }

    #[test]
    fn test_loops_none_in_if_else() {
        let cfg = TestCfg::if_else();
        let loops = cfg.detect_loops();

        assert_eq!(loops.len(), 0);
    }
}
