use std::cmp::Ordering;

use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, PartialEq)]
struct QSig {
    sig: Vec<i32>,
    pos: usize,
}

pub struct IndexGram {
    q: usize,
    q_grams: Vec<Vec<QSig>>,
    inv_index: FxHashMap<Vec<i32>, Vec<(usize, usize, usize)>>,
    ordering: FxHashMap<Vec<i32>, i32>,
}

impl IndexGram {
    pub const EMPTY_VALUE: i32 = -1;
    pub fn new(data: &[Vec<i32>], q: usize) -> Self {
        let mut q_grams = vec![];

        for mut sdata in data.iter().cloned() {
            let sig_size = sdata.len().div_ceil(q);
            sdata.append(&mut vec![Self::EMPTY_VALUE; sig_size * q - sdata.len()]);

            let sqgrams: Vec<QSig> = sdata
                .windows(q)
                .enumerate()
                .map(|(i, w)| QSig {
                    sig: w.to_vec(),
                    pos: i,
                })
                .collect();
            q_grams.push(sqgrams);
        }

        let mut frequency_map = FxHashMap::default();

        for qgrams in q_grams.iter() {
            for qgram in qgrams.iter().cloned() {
                frequency_map
                    .entry(qgram.sig)
                    .and_modify(|cnt| *cnt += 1)
                    .or_insert(1);
            }
        }

        let mut inv_index = FxHashMap::default();

        for str_grams in q_grams.iter_mut() {
            str_grams.sort_by(|ag, bg| {
                let ord = frequency_map.get(&ag.sig).cmp(&frequency_map.get(&bg.sig));
                if ord == Ordering::Equal {
                    ag.pos.cmp(&bg.pos)
                } else {
                    ord
                }
            });
        }

        for (sid, str_grams) in q_grams.iter().enumerate() {
            for (gram_pos, gram) in str_grams.iter().cloned().enumerate() {
                inv_index
                    .entry(gram.sig)
                    .and_modify(|postings: &mut Vec<(usize, usize, usize)>| {
                        postings.push((sid, data[sid].len(), gram_pos))
                    })
                    .or_insert(vec![(sid, data[sid].len(), gram_pos)]);
            }
        }

        IndexGram {
            q,
            q_grams,
            ordering: frequency_map,
            inv_index,
        }
    }

    pub fn query(&self, mut query: Vec<i32>, k: usize) -> Result<Vec<usize>, String> {
        let sig_size = query.len().div_ceil(self.q);
        if k > sig_size {
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

        chunks.sort_by_cached_key(|chunk| self.ordering.get(&chunk.sig).unwrap_or(&i32::MAX));
        let mut cs = FxHashSet::default();
        for (chunk_pos, chunk) in chunks.iter().enumerate() {
            let chunk_pos = chunk_pos * self.q;
            if let Some(postings) = self.inv_index.get(&chunk.sig) {
                for (cid, slen, gram_pos) in postings.iter() {
                    if slen.abs_diff(query.len()) <= k && chunk_pos.abs_diff(*gram_pos) <= k {
                        cs.insert(*cid);
                    }
                }
            }
        }

        // count filter
        Ok(cs
            .iter()
            .filter(|cid| {
                let mut mismatch = 0;
                let mut candidate_gram_matches = vec![];
                let lb = sig_size - k;
                let candidate_grams = &self.q_grams[**cid];
                // dbg!(candidate_grams);
                // dbg!(chunks.iter().enumerate().map(|(mi, chk)| (mi * self.q, chk) ).collect::<Vec<_>>());
                for chunk in chunks.iter() {
                    let chunk_match = candidate_grams
                        .iter()
                        .position(|gram| gram.sig == chunk.sig);
                    if let Some(mut match_idx) = chunk_match {
                        while match_idx < candidate_grams.len()
                            && candidate_grams[match_idx].sig == chunk.sig
                            && chunk.pos.abs_diff(candidate_grams[match_idx].pos) <= k
                        {
                            candidate_gram_matches.push((chunk, match_idx));
                            match_idx += 1;
                        }
                    } else {
                        mismatch += 1;
                        if mismatch > chunks.len() - lb {
                            return false;
                        }
                    }
                }
                candidate_gram_matches.len() >= lb
            })
            .cloned()
            .collect::<Vec<usize>>())
    }
}
