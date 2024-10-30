use std::{
    cmp::Ordering,
    time::{Duration, Instant},
};

use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
struct QSig {
    sig: Vec<i32>,
    pos: usize,
}

pub struct IndexGram {
    q: usize,
    q_grams: Vec<(usize, Vec<QSig>)>,
    inv_index: FxHashMap<Vec<i32>, Vec<(usize, usize)>>,
    ordering: FxHashMap<Vec<i32>, i32>,
}

impl IndexGram {
    pub const EMPTY_VALUE: i32 = -1;
    pub fn new(data: &[Vec<i32>], q: usize) -> Self {
        let mut q_grams = vec![];

        for mut sdata in data.iter().cloned() {
            let sig_size = sdata.len().div_ceil(q);
            sdata.append(&mut vec![Self::EMPTY_VALUE; sig_size * q - sdata.len()]);

            let mut sqgrams: Vec<QSig> = sdata
                .windows(q)
                .enumerate()
                .map(|(i, w)| QSig {
                    sig: w.to_vec(),
                    pos: i,
                })
                .collect();
            sqgrams.sort();
            q_grams.push((sdata.len(), sqgrams));
        }

        let mut frequency_map = FxHashMap::default();

        // for qgrams in q_grams.iter() {
        //     for qgram in qgrams.iter().cloned() {
        //         frequency_map
        //             .entry(qgram.sig)
        //             .and_modify(|cnt| *cnt += 1)
        //             .or_insert(1);
        //     }
        // }

        let mut inv_index = FxHashMap::default();

        // for str_grams in q_grams.iter_mut() {
        //     str_grams.sort_by(|ag, bg| {
        //         let ord = frequency_map.get(&ag.sig).cmp(&frequency_map.get(&bg.sig));
        //         if ord == Ordering::Equal {
        //             ag.pos.cmp(&bg.pos)
        //         } else {
        //             ord
        //         }
        //     });
        // }

        for (sid, str_grams) in q_grams.iter().enumerate() {
            for (gram_pos, gram) in str_grams.1.iter().cloned().enumerate() {
                inv_index
                    .entry(gram.sig)
                    .and_modify(|postings: &mut Vec<(usize, usize)>| postings.push((sid, gram_pos)))
                    .or_insert(vec![(sid, gram_pos)]);
            }
        }

        IndexGram {
            q,
            q_grams,
            ordering: frequency_map,
            inv_index,
        }
    }

    pub fn query(
        &self,
        mut query: Vec<i32>,
        k: usize,
    ) -> Result<(Vec<usize>, Duration, Duration), String> {
        let sig_size = query.len().div_ceil(self.q);
        if k >= sig_size {
            // eprintln!(
            //     "{k} > {}, output may have false negatives! lb={lb}",
            //     chunks.len(),
            //     lb = sig_size - k
            // );
            return Err("Query is too small for that threshold!".to_owned());
        }
        query.append(&mut vec![
            Self::EMPTY_VALUE;
            sig_size * self.q - query.len()
        ]);
        let mut chunks: Vec<QSig> = query
            .chunks(self.q)
            .enumerate()
            .map(|(pos, c)| QSig {
                sig: c.to_vec(),
                pos: pos * self.q,
            })
            .collect();

        chunks.sort_by_key(|c| c.pos);
        // chunks.sort_by_cached_key(|chunk| self.ordering.get(&chunk.sig).unwrap_or(&i32::MAX));
        let index_lookup = Instant::now();
        let mut cs = FxHashSet::default();
        for chunk in chunks.iter() {
            if let Some(postings) = self.inv_index.get(&chunk.sig) {
                for (cid, gram_pos) in postings.iter() {
                    let slen = self.q_grams[*cid].0;
                    if slen.abs_diff(query.len()) <= k && chunk.pos.abs_diff(*gram_pos) <= k {
                        cs.insert(*cid);
                    }
                }
            }
        }

        let index_lookup_dur = index_lookup.elapsed();
        let filter_time = Instant::now();

        // count filter
        // TODO: This count filter is taking an absurdly long time to complete...
        let candidates = cs
            .iter()
            .filter(|cid| self.count_filter(**cid, sig_size, k, &chunks))
            .cloned()
            .collect::<Vec<usize>>();
        let filter_duration = filter_time.elapsed();

        // let candidates = cs.iter().cloned().collect_vec();

        Ok((candidates, index_lookup_dur, filter_duration))
    }

    fn count_filter(&self, cid: usize, sig_size: usize, k: usize, chunks: &[QSig]) -> bool {
        let mut mismatch = 0;
        let mut candidate_gram_matches = vec![];
        // let mut candidate_gram_matches = 0;

        let lb = sig_size - k;
        let candidate_grams = &self.q_grams[cid].1;
        // dbg!(candidate_grams);
        // dbg!(chunks.iter().enumerate().map(|(mi, chk)| (mi * self.q, chk) ).collect::<Vec<_>>());
        for chunk in chunks.iter() {
            match candidate_grams.binary_search_by(|probe| match probe.sig.cmp(&chunk.sig) {
                std::cmp::Ordering::Equal => probe
                    .pos
                    .cmp(&chunk.pos.saturating_sub(k))
                    .then(std::cmp::Ordering::Greater),
                other => other,
            }) {
                Err(mut match_idx) => {
                    if match_idx >= candidate_grams.len()
                        || candidate_grams[match_idx].sig != chunk.sig
                    {
                        mismatch += 1;
                        if mismatch > chunks.len() - lb {
                            return false;
                        }
                    } else {
                        while match_idx < candidate_grams.len()
                            && candidate_grams[match_idx].sig == chunk.sig
                            && chunk.pos.abs_diff(candidate_grams[match_idx].pos) <= k
                        {
                            candidate_gram_matches.push((chunk, &candidate_grams[match_idx]));
                            match_idx += 1;
                        }
                    }
                }
                Ok(_) => {}
            }
        }

        // return candidate_gram_matches.len() >= lb;

        if candidate_gram_matches.len() < lb {
            return false;
        }

        // candidate_gram_matches.sort_by(|a, b| a.0.pos.cmp(&b.0.pos));

        // true match filter
        let omni_match = QSig {
            sig: vec![-1, -1],
            pos: usize::MAX,
        };
        candidate_gram_matches.insert(0, (&omni_match, &omni_match));
        let mut opt = vec![0; candidate_gram_matches.len()];

        // the first in tuple is the q-chunk of query, second is q-gram of data string
        #[inline(always)]
        fn compatible(m1: (&QSig, &QSig), m2: (&QSig, &QSig), n: usize) -> bool {
            if m2.0.sig[0] == -1 {
                return true;
            }
            m1.0 != m2.0 && m1.1.pos >= m2.1.pos + n // return value of compatible
        }

        for k in 1..candidate_gram_matches.len() {
            let mut mx = i32::MIN;
            let mn = std::cmp::min(k, candidate_gram_matches.len() - lb + 1);
            for i in 1..=mn {
                if compatible(
                    candidate_gram_matches[k],
                    candidate_gram_matches[k - i],
                    self.q,
                ) && opt[k - i] > mx
                {
                    mx = opt[k - i] + 1;
                }
            }
            opt[k] = mx;
        }

        opt.iter().skip(lb).max().unwrap() >= &(lb as i32)
    }

    fn strlen_from_qgrams(&self, grams: &[QSig]) -> usize {
        if grams.last().unwrap().sig.last().unwrap() == &Self::EMPTY_VALUE {
            return grams.len();
        }
        grams.len() + (self.q - 1)
    }
}
