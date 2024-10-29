pub mod binary_branch;
pub mod indexes;
pub mod label_intersection;
pub mod sed;
pub mod structural_filter;

macro_rules! iterate_queries {
    ($query_tuple:ident, $tree_indexes:ident, $lb_func:ident) => {{
        let __start_time = std::time::Instant::now();
        let mut candidates = vec![];
        for (qid, (t, query)) in $query_tuple.iter().enumerate() {
            for (tid, tree) in $tree_indexes.iter().enumerate() {
                if $lb_func(query, tree, *t) <= *t {
                    candidates.push((qid, tid));
                }
            }
        }

        (candidates, __start_time.elapsed())
    }};
    ($query_tuple:ident, $tree_indexes:ident, $lb_func:ident, $size_map:ident) => {{
        let __start_time = std::time::Instant::now();
        let mut candidates = vec![];
        let trees_len = $tree_indexes.len();
        for (qid, (t, query)) in $query_tuple.iter().enumerate() {
            let start_idx = $size_map
                .get(&query.c.tree_size.saturating_sub(*t))
                .unwrap_or(&0);
            let end_idx = $size_map
                .get(&(query.c.tree_size + t + 1))
                .unwrap_or(&trees_len);
            let idx_diff = end_idx - start_idx;
            // println!("Starting from {start_idx} and taking at most {idx_diff} trees!");

            for (tid, tree) in $tree_indexes
                .iter()
                .enumerate()
                .skip(*start_idx)
                .take(idx_diff)
            {
                if $lb_func(query, tree, *t) <= *t {
                    candidates.push((qid, tid));
                }
            }
        }

        (candidates, __start_time.elapsed())
    }};
}

pub(crate) use iterate_queries;
