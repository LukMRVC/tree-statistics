// The MIT License (MIT)
// Copyright (c) 2017 Mateusz Pawlik.
//

/*! Implements the state-of-the-art tree edit distance algorithm APTED+ by
 Pawlik and Augsten [1,2,3,4].

 [1] M.Pawlik and N.Augsten. RTED: A Robust Algorithm for the Tree Edit
     Distance. PVLDB. 2011.

 [2] M.Pawlik and N.Augsten. A Memory-Efficient Tree Edit Distance Algorithm.
     DEXA. 2014.

 [3] M. Pawlik and N. Augsten. Efficient Computation of the Tree Edit
     Distance. ACM Transactions on Database Systems (TODS). 2015.

 [4] M. Pawlik and N. Augsten. Tree edit distance: Robust and
     memory-efficient. Information Systems. 2016.

 NOTE: only node::TreeIndexAPTED can be used with APTED.
!*/

use crate::indexing::AptedIndex;

pub struct Apted {

}

impl Apted {
    pub fn ted(t1: &AptedIndex, t2: &AptedIndex) -> usize {
        let (size1, size2) = (t1.c.tree_size, t2.c.tree_size);
        let (rows, columns) = (size1, size2);
        let at = |row: usize, col: usize| -> usize {
            row * columns + col
        };
        let mut strategy = Vec::with_capacity(rows * columns);
        let mut strategy_path = -1.0;
        let mut min_cost = i64::MAX;
        // initialize cost vectors
        let mut cost1_l = Vec::with_capacity(size1);
        let mut cost1_r = Vec::with_capacity(size1);
        let mut cost1_i = Vec::with_capacity(size1);
        let mut cost2_l = Vec::<i64>::with_capacity(size2);
        let mut cost2_r = Vec::<i64>::with_capacity(size2);
        let mut cost2_i = Vec::<i64>::with_capacity(size2);
        let mut cost2_path = Vec::<f64>::with_capacity(size2);

        let mut leaf_row = Vec::<i64>::with_capacity(size2);
        let path_id_offset = size1 as f64;

        let pre2size1 = &t1.prel_to_size_;
        let pre2size2 = &t2.prel_to_size_;
        let pre2desc_sum1 = &t1.prel_to_cost_all_;
        let pre2desc_sum2 = &t2.prel_to_cost_all_;
        let pre2kr_sum1 = &t1.prel_to_cost_left_;
        let pre2kr_sum2 = &t2.prel_to_cost_left_;
        let pre2revkr_sum1 = &t1.prel_to_cost_right_;
        let pre2revkr_sum2 = &t2.prel_to_cost_right_;
        let pre_l_to_pre_r_1 = &t1.prel_to_prer_;
        let pre_l_to_pre_r_2 = &t2.prel_to_prer_;
        let pre_r_to_pre_l_1 = &t1.prer_to_prel_;
        let pre_r_to_pre_l_2 = &t2.prer_to_prel_;
        let pre2parent1 = &t1.prel_to_parent_;
        let pre2parent2 = &t2.prel_to_parent_;
        let node_type_l_1 = &t1.prel_to_type_left_;
        let node_type_l_2 = &t2.prel_to_type_left_;
        let node_type_r_1 = &t1.prel_to_type_right_;
        let node_type_r_2 = &t2.prel_to_type_right_;
        let pre_l_to_post_l_1 = &t1.prel_to_postl_;
        let pre_l_to_post_l_2 = &t2.prel_to_postl_;
        let post_l_to_pre_l_1 = &t1.postl_to_prel_;
        let post_l_to_pre_l_2 = &t2.postl_to_prel_;


        0
    }
}

