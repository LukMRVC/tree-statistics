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
}

pub(crate) use iterate_queries;
