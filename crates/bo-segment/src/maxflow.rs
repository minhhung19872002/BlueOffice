//! Minimum s–t cut by maximum flow, with the two-search-tree augmenting path method
//! described by Y. Boykov and V. Kolmogorov ("An Experimental Comparison of
//! Min-Cut/Max-Flow Algorithms for Energy Minimization in Vision", IEEE TPAMI 26(9),
//! 2004): a source tree and a sink tree grow from the terminals until they touch, the
//! path found is augmented, and the nodes cut off from their tree ("orphans") look
//! for a new parent or become free. Written from the published description.
//!
//! Arcs come in pairs (`a` and its reverse `a ^ 1`); every node keeps a residual
//! terminal capacity `tr` (positive: from the source, negative: to the sink).

use std::collections::VecDeque;

const NONE: u32 = u32::MAX;
const TERMINAL: u32 = u32::MAX - 1;
const ORPHAN: u32 = u32::MAX - 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tree {
    Free,
    Source,
    Sink,
}

/// A flow network with integer capacities.
#[derive(Debug, Clone)]
pub struct Graph {
    first: Vec<u32>,
    head: Vec<u32>,
    next: Vec<u32>,
    cap: Vec<i32>,
    tr: Vec<i64>,
    flow: i64,
    tree: Vec<Tree>,
    parent: Vec<u32>,
    ts: Vec<u32>,
    dist: Vec<u32>,
    active: Vec<bool>,
}

impl Graph {
    /// A graph of `nodes` nodes with room for `edges` edges.
    pub fn new(nodes: usize, edges: usize) -> Graph {
        Graph {
            first: vec![NONE; nodes],
            head: Vec::with_capacity(edges * 2),
            next: Vec::with_capacity(edges * 2),
            cap: Vec::with_capacity(edges * 2),
            tr: vec![0; nodes],
            flow: 0,
            tree: vec![Tree::Free; nodes],
            parent: vec![NONE; nodes],
            ts: vec![0; nodes],
            dist: vec![0; nodes],
            active: vec![false; nodes],
        }
    }

    /// Number of nodes.
    pub fn nodes(&self) -> usize {
        self.first.len()
    }

    /// Edge `u → v` with capacity `cap_uv` and `v → u` with `cap_vu` (both ≥ 0).
    pub fn add_edge(&mut self, u: usize, v: usize, cap_uv: i32, cap_vu: i32) {
        debug_assert!(u != v && cap_uv >= 0 && cap_vu >= 0);
        let a = self.head.len() as u32;
        self.head.push(v as u32);
        self.next.push(self.first[u]);
        self.cap.push(cap_uv.max(0));
        self.first[u] = a;
        self.head.push(u as u32);
        self.next.push(self.first[v]);
        self.cap.push(cap_vu.max(0));
        self.first[v] = a + 1;
    }

    /// Add capacities source → `u` and `u` → sink (both ≥ 0).
    pub fn add_tweights(&mut self, u: usize, source: i64, sink: i64) {
        let (mut s, mut t) = (source.max(0), sink.max(0));
        let d = self.tr[u];
        if d > 0 {
            s += d;
        } else {
            t -= d;
        }
        self.flow += s.min(t);
        self.tr[u] = s - t;
    }

    fn activate(&mut self, queue: &mut VecDeque<u32>, u: u32) {
        if !self.active[u as usize] {
            self.active[u as usize] = true;
            queue.push_back(u);
        }
    }

    /// Run the maximum flow; returns its value (the capacity of the minimum cut).
    pub fn maxflow(&mut self) -> i64 {
        let n = self.nodes();
        // Greedy start: push the paths s → u → v → t of one arc directly (most of the
        // flow of picture graphs), so the trees only carry what remains.
        for u in 0..n {
            let mut a = self.first[u];
            while a != NONE && self.tr[u] > 0 {
                let ai = a as usize;
                let v = self.head[ai] as usize;
                if self.tr[v] < 0 && self.cap[ai] > 0 {
                    let b = self.tr[u].min(-self.tr[v]).min(i64::from(self.cap[ai]));
                    self.tr[u] -= b;
                    self.tr[v] += b;
                    self.cap[ai] -= b as i32;
                    self.cap[ai ^ 1] += b as i32;
                    self.flow += b;
                }
                a = self.next[ai];
            }
        }
        let mut queue = VecDeque::new();
        for u in 0..n {
            self.active[u] = false;
            self.ts[u] = 0;
            if self.tr[u] == 0 {
                self.tree[u] = Tree::Free;
                self.parent[u] = NONE;
            } else {
                self.tree[u] = if self.tr[u] > 0 {
                    Tree::Source
                } else {
                    Tree::Sink
                };
                self.parent[u] = TERMINAL;
                self.dist[u] = 1;
                self.activate(&mut queue, u as u32);
            }
        }
        let mut time = 0u32;
        let mut orphans = VecDeque::new();
        loop {
            // Growth: find an arc joining the two trees.
            let mut found = NONE;
            while let Some(&p) = queue.front() {
                let pu = p as usize;
                if self.tree[pu] == Tree::Free {
                    queue.pop_front();
                    self.active[pu] = false;
                    continue;
                }
                let mut a = self.first[pu];
                while a != NONE {
                    let ai = a as usize;
                    let q = self.head[ai] as usize;
                    if self.tree[pu] == Tree::Source {
                        if self.cap[ai] > 0 {
                            match self.tree[q] {
                                Tree::Free => {
                                    self.adopt_free(&mut queue, q, pu, a ^ 1, Tree::Source);
                                }
                                Tree::Sink => {
                                    found = a;
                                    break;
                                }
                                Tree::Source => self.shorten(q, pu, a ^ 1),
                            }
                        }
                    } else if self.cap[ai ^ 1] > 0 {
                        match self.tree[q] {
                            Tree::Free => {
                                self.adopt_free(&mut queue, q, pu, a ^ 1, Tree::Sink);
                            }
                            Tree::Source => {
                                found = a ^ 1;
                                break;
                            }
                            Tree::Sink => self.shorten(q, pu, a ^ 1),
                        }
                    }
                    a = self.next[ai];
                }
                if found != NONE {
                    break;
                }
                queue.pop_front();
                self.active[pu] = false;
            }
            if found == NONE {
                break;
            }
            time += 1;
            self.augment(found as usize, &mut orphans);
            self.adopt(&mut queue, &mut orphans, time);
        }
        self.flow
    }

    fn adopt_free(&mut self, queue: &mut VecDeque<u32>, q: usize, p: usize, arc: u32, t: Tree) {
        self.tree[q] = t;
        self.parent[q] = arc;
        self.ts[q] = self.ts[p];
        self.dist[q] = self.dist[p] + 1;
        self.activate(queue, q as u32);
    }

    /// Growth heuristic of the method: a tree neighbour q whose distance to the
    /// terminal is known to be longer re-parents to p (keeps the trees shallow).
    fn shorten(&mut self, q: usize, p: usize, arc: u32) {
        if self.ts[q] <= self.ts[p] && self.dist[q] > self.dist[p] {
            self.parent[q] = arc;
            self.ts[q] = self.ts[p];
            self.dist[q] = self.dist[p] + 1;
        }
    }

    fn orphan(&mut self, orphans: &mut VecDeque<u32>, x: usize) {
        self.parent[x] = ORPHAN;
        orphans.push_back(x as u32);
    }

    /// Push the bottleneck along source root → … → middle arc `m` → … → sink root.
    fn augment(&mut self, m: usize, orphans: &mut VecDeque<u32>) {
        let mut b = i64::from(self.cap[m]);
        let mut x = self.head[m ^ 1] as usize;
        loop {
            let a = self.parent[x];
            if a == TERMINAL {
                b = b.min(self.tr[x]);
                break;
            }
            b = b.min(i64::from(self.cap[a as usize ^ 1]));
            x = self.head[a as usize] as usize;
        }
        x = self.head[m] as usize;
        loop {
            let a = self.parent[x];
            if a == TERMINAL {
                b = b.min(-self.tr[x]);
                break;
            }
            b = b.min(i64::from(self.cap[a as usize]));
            x = self.head[a as usize] as usize;
        }
        let bi = b as i32;
        self.cap[m] -= bi;
        self.cap[m ^ 1] += bi;
        x = self.head[m ^ 1] as usize;
        loop {
            let a = self.parent[x];
            if a == TERMINAL {
                self.tr[x] -= b;
                if self.tr[x] == 0 {
                    self.orphan(orphans, x);
                }
                break;
            }
            let ai = a as usize;
            self.cap[ai ^ 1] -= bi;
            self.cap[ai] += bi;
            if self.cap[ai ^ 1] == 0 {
                self.orphan(orphans, x);
            }
            x = self.head[ai] as usize;
        }
        x = self.head[m] as usize;
        loop {
            let a = self.parent[x];
            if a == TERMINAL {
                self.tr[x] += b;
                if self.tr[x] == 0 {
                    self.orphan(orphans, x);
                }
                break;
            }
            let ai = a as usize;
            self.cap[ai] -= bi;
            self.cap[ai ^ 1] += bi;
            if self.cap[ai] == 0 {
                self.orphan(orphans, x);
            }
            x = self.head[ai] as usize;
        }
        self.flow += b;
    }

    /// Residual capacity from a node of tree `t` towards its parent side through arc `a`
    /// (leaving the child): the source tree carries flow parent → child.
    fn tree_cap(&self, t: Tree, a: usize) -> i32 {
        if t == Tree::Source {
            self.cap[a ^ 1]
        } else {
            self.cap[a]
        }
    }

    /// Distance of `y` to its terminal when its path is still rooted (marks the path).
    fn rooted_dist(&mut self, y: usize, time: u32) -> Option<u32> {
        let mut j = y;
        let mut d = 0u32;
        loop {
            if self.ts[j] == time {
                d += self.dist[j];
                break;
            }
            let pa = self.parent[j];
            d += 1;
            if pa == TERMINAL {
                self.ts[j] = time;
                self.dist[j] = 1;
                break;
            }
            if pa == ORPHAN || pa == NONE {
                return None;
            }
            j = self.head[pa as usize] as usize;
        }
        // Cache the distances along the path.
        let mut j = y;
        let mut dd = d;
        while self.ts[j] != time {
            self.ts[j] = time;
            self.dist[j] = dd;
            dd = dd.saturating_sub(1);
            j = self.head[self.parent[j] as usize] as usize;
        }
        Some(d)
    }

    fn adopt(&mut self, queue: &mut VecDeque<u32>, orphans: &mut VecDeque<u32>, time: u32) {
        while let Some(x) = orphans.pop_front() {
            let x = x as usize;
            let t = self.tree[x];
            let mut best = NONE;
            let mut dmin = u32::MAX;
            if (t == Tree::Source && self.tr[x] > 0) || (t == Tree::Sink && self.tr[x] < 0) {
                best = TERMINAL;
                dmin = 0;
            } else {
                let mut a = self.first[x];
                while a != NONE {
                    let ai = a as usize;
                    let y = self.head[ai] as usize;
                    if self.tree[y] == t && self.tree_cap(t, ai) > 0 {
                        if let Some(d) = self.rooted_dist(y, time) {
                            if d < dmin {
                                best = a;
                                dmin = d;
                            }
                        }
                    }
                    a = self.next[ai];
                }
            }
            if best != NONE {
                self.parent[x] = best;
                self.ts[x] = time;
                self.dist[x] = dmin + 1;
                continue;
            }
            // No parent: x leaves its tree; its children become orphans, its
            // neighbours that could reach it become active again.
            let mut a = self.first[x];
            while a != NONE {
                let ai = a as usize;
                let y = self.head[ai] as usize;
                if self.tree[y] == t {
                    if self.tree_cap(t, ai) > 0 {
                        self.activate(queue, y as u32);
                    }
                    let pa = self.parent[y];
                    if pa != TERMINAL
                        && pa != ORPHAN
                        && pa != NONE
                        && self.head[pa as usize] as usize == x
                    {
                        self.orphan(orphans, y);
                    }
                }
                a = self.next[ai];
            }
            self.tree[x] = Tree::Free;
            self.parent[x] = NONE;
        }
    }

    /// After [`Graph::maxflow`]: `u` is on the source side of the minimum cut.
    pub fn is_source(&self, u: usize) -> bool {
        self.tree[u] == Tree::Source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference maximum flow (Edmonds–Karp on a dense matrix; node 0 = source,
    /// node n-1 = sink).
    fn edmonds_karp(mut c: Vec<Vec<i64>>) -> i64 {
        let n = c.len();
        let mut flow = 0;
        loop {
            let mut prev = vec![usize::MAX; n];
            prev[0] = 0;
            let mut q = VecDeque::from([0usize]);
            while let Some(u) = q.pop_front() {
                for v in 0..n {
                    if prev[v] == usize::MAX && c[u][v] > 0 {
                        prev[v] = u;
                        q.push_back(v);
                    }
                }
            }
            if prev[n - 1] == usize::MAX {
                return flow;
            }
            let mut b = i64::MAX;
            let mut v = n - 1;
            while v != 0 {
                b = b.min(c[prev[v]][v]);
                v = prev[v];
            }
            let mut v = n - 1;
            while v != 0 {
                c[prev[v]][v] -= b;
                c[v][prev[v]] += b;
                v = prev[v];
            }
            flow += b;
        }
    }

    #[test]
    fn two_node_cut() {
        // s →3→ a →2→ b →4→ t (a–b edge is the bottleneck).
        let mut g = Graph::new(2, 1);
        g.add_tweights(0, 3, 0);
        g.add_tweights(1, 0, 4);
        g.add_edge(0, 1, 2, 0);
        assert_eq!(g.maxflow(), 2);
        assert!(g.is_source(0));
        assert!(!g.is_source(1));
    }

    #[test]
    fn classic_example() {
        // CLRS figure 26.1: max flow 23 (s, v1..v4, t; s and t as terminal capacities).
        // s→v1 16, s→v2 13, v1→v3 12, v2→v1 4, v2→v4 14, v3→v2 9, v3→t 20, v4→v3 7, v4→t 4.
        let mut g = Graph::new(4, 5);
        g.add_tweights(0, 16, 0);
        g.add_tweights(1, 13, 0);
        g.add_tweights(2, 0, 20);
        g.add_tweights(3, 0, 4);
        g.add_edge(0, 2, 12, 0);
        g.add_edge(1, 0, 4, 0);
        g.add_edge(1, 3, 14, 0);
        g.add_edge(2, 1, 9, 0);
        g.add_edge(3, 2, 7, 0);
        assert_eq!(g.maxflow(), 23);
        // Known minimum cut {s, v1, v2, v4} | {v3, t}.
        assert!(g.is_source(0) && g.is_source(1) && g.is_source(3));
        assert!(!g.is_source(2));
    }

    #[test]
    fn both_terminals_on_one_node() {
        let mut g = Graph::new(1, 0);
        g.add_tweights(0, 5, 3);
        assert_eq!(g.maxflow(), 3);
        assert!(g.is_source(0));
    }

    /// Small pseudo-random graphs: flow equals Edmonds–Karp, and the cut found has
    /// exactly that capacity.
    #[test]
    fn matches_reference_on_random_graphs() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m
        };
        for _ in 0..300 {
            let n = 2 + rnd(9) as usize;
            let mut dense = vec![vec![0i64; n + 2]; n + 2];
            let mut g = Graph::new(n, n * n);
            let mut caps = Vec::new();
            for u in 0..n {
                let (s, t) = (
                    rnd(4) as i64 * rnd(10) as i64,
                    rnd(4) as i64 * rnd(10) as i64,
                );
                g.add_tweights(u, s, t);
                dense[0][u + 1] += s;
                dense[u + 1][n + 1] += t;
            }
            for u in 0..n {
                for v in u + 1..n {
                    if rnd(3) == 0 {
                        let (a, b) = (rnd(8) as i32, rnd(8) as i32);
                        g.add_edge(u, v, a, b);
                        dense[u + 1][v + 1] += i64::from(a);
                        dense[v + 1][u + 1] += i64::from(b);
                        caps.push((u, v, a, b));
                    }
                }
            }
            let want = edmonds_karp(dense.clone());
            let got = g.maxflow();
            assert_eq!(got, want);
            // Capacity of the cut (S = source side).
            let mut cut = 0i64;
            for u in 0..n {
                if g.is_source(u) {
                    cut += dense[u + 1][n + 1];
                } else {
                    cut += dense[0][u + 1];
                }
            }
            for (u, v, a, b) in caps {
                if g.is_source(u) && !g.is_source(v) {
                    cut += i64::from(a);
                }
                if g.is_source(v) && !g.is_source(u) {
                    cut += i64::from(b);
                }
            }
            assert_eq!(cut, want);
        }
    }

    #[test]
    fn grid_cut_follows_the_weak_column() {
        // 4×4 grid, strong edges except between columns 1 and 2; left column tied to the
        // source, right column to the sink: the cut runs between columns 1 and 2.
        let (w, h) = (4usize, 4usize);
        let mut g = Graph::new(w * h, 2 * w * h);
        for y in 0..h {
            for x in 0..w {
                let u = y * w + x;
                if x + 1 < w {
                    let c = if x == 1 { 1 } else { 100 };
                    g.add_edge(u, u + 1, c, c);
                }
                if y + 1 < h {
                    g.add_edge(u, u + w, 100, 100);
                }
            }
            g.add_tweights(y * w, 1000, 0);
            g.add_tweights(y * w + w - 1, 0, 1000);
        }
        assert_eq!(g.maxflow(), 4);
        for y in 0..h {
            for x in 0..w {
                assert_eq!(g.is_source(y * w + x), x < 2, "({x},{y})");
            }
        }
    }
}
