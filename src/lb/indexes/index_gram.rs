use std::time::{Duration, Instant};

use itertools::Itertools;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
struct QSig {
    sig: Vec<i32>,
    pos: i32,
}

pub struct IndexGram {
    q: usize,
    // q_grams: Vec<(usize, Vec<QSig>)>,
    inv_index: FxHashMap<Vec<i32>, Vec<(usize, i32, i32)>>,
    pub true_matches: Duration,
    pub cnt: Duration,
}

impl IndexGram {
    pub const EMPTY_VALUE: i32 = i32::MAX;
    pub fn new(data: &[Vec<i32>], q: usize) -> Self {
        let mut inv_index = FxHashMap::default();

        for (sid, mut sdata) in data.iter().cloned().enumerate() {
            let sig_size = sdata.len().div_ceil(q);
            let orig_len = sdata.len() as i32;
            sdata.append(&mut vec![Self::EMPTY_VALUE; sig_size * q - sdata.len()]);

            sdata.windows(q).enumerate().for_each(|(i, w)| {
                inv_index
                    .entry(w.to_vec())
                    .and_modify(|postings: &mut Vec<(usize, i32, i32)>| {
                        postings.push((sid, orig_len, i as i32))
                    })
                    .or_insert(vec![(sid, orig_len, i as i32)]);
            });
        }

        IndexGram {
            q,
            // q_grams,
            inv_index,
            cnt: Duration::from_micros(0),
            true_matches: Duration::from_micros(0),
        }
    }

    pub fn query(
        &self,
        mut query: Vec<i32>,
        k: usize,
    ) -> Result<(Vec<usize>, Duration, Duration), String> {
        let index_lookup = Instant::now();

        let sig_size = query.len().div_ceil(self.q);
        let min_allowed_sig_size = query.len() / self.q;
        if k >= min_allowed_sig_size {
            // eprintln!(
            //     "{k} > {}, output may have false negatives! lb={lb}",
            //     chunks.len(),
            //     lb = sig_size - k
            // );
            return Err("Query is too small for that threshold!".to_owned());
        }
        let min_match_size = query.len().saturating_sub(k) as i32;
        let max_match_size = (query.len() + k + 1) as i32;
        query.append(&mut vec![
            Self::EMPTY_VALUE;
            sig_size * self.q - query.len()
        ]);

        let chunks: Vec<QSig> = query
            .chunks(self.q)
            .enumerate()
            .map(|(pos, c)| QSig {
                sig: c.to_vec(),
                pos: (pos * self.q) as i32,
            })
            .collect();
        let mut cs = FxHashMap::default();

        // for chunk in chunks.iter().take(k + 1)
        for chunk in chunks.iter() {
            // dbg!(chunk);
            if let Some(postings) = self.inv_index.get(&chunk.sig) {
                let Err(start) = postings.binary_search_by(|probe| {
                    probe
                        .1
                        .cmp(&min_match_size)
                        .then(std::cmp::Ordering::Greater)
                }) else {
                    panic!("Binary search cannot result in Ok!");
                };
                let Err(end) = postings.binary_search_by(|probe| {
                    probe
                        .1
                        .cmp(&max_match_size)
                        .then(std::cmp::Ordering::Greater)
                }) else {
                    panic!("Binary search cannot result in Ok!");
                };
                let to_take = end - start;
                for (cid, _, gram_pos) in postings.iter().skip(start).take(to_take) {
                    if chunk.pos.abs_diff(*gram_pos) <= (k as u32) {
                        cs.entry(*cid)
                            .and_modify(|candidate_grams: &mut Vec<(&QSig, i32)>| {
                                candidate_grams.push((chunk, *gram_pos))
                            })
                            .or_insert(vec![(chunk, *gram_pos)]);
                        // cs.insert(*cid);
                    }
                }
            }
        }

        let index_lookup_dur = index_lookup.elapsed();
        let filter_time = Instant::now();
        let lb: usize = sig_size - k;
        let mut opt = vec![0; 128];
        // count and true matches filter
        let candidates = cs
            .into_iter()
            .filter_map(|(cid, mut candidate_gram_matches)| {
                if candidate_gram_matches.len() < lb {
                    return None;
                }
                candidate_gram_matches.sort_by_key(|(chunk, _)| chunk.pos);

                // true match filter
                let omni_match = QSig {
                    sig: vec![-1],
                    pos: i32::MAX,
                };
                candidate_gram_matches.insert(0, (&omni_match, omni_match.pos));
                // let mut opt = vec![0; candidate_gram_matches.len()];
                opt.fill(0);

                if opt.len() < candidate_gram_matches.len() {
                    opt.resize(candidate_gram_matches.len(), 0);
                }

                #[inline(always)]
                fn compatible(m1: &(&QSig, i32), m2: &(&QSig, i32), n: i32) -> bool {
                    *unsafe { m2.0.sig.get_unchecked(0) } == -1
                        || ((m1.0.pos != m2.0.pos && m1.0.sig != m2.0.sig) && m1.1 >= m2.1 + n)
                }

                let qsize = self.q as i32;
                unsafe {
                    // the first in tuple is the q-chunk of query, second is q-gram of data string

                    let mut total_max = i32::MIN;
                    for kc in 1..candidate_gram_matches.len() {
                        let mut mx = i32::MIN;
                        let mn = std::cmp::min(kc, candidate_gram_matches.len() - lb + 1);
                        for i in 1..=mn {
                            if *opt.get_unchecked(kc - i) > mx
                                && compatible(
                                    candidate_gram_matches.get_unchecked(kc),
                                    candidate_gram_matches.get_unchecked(kc - i),
                                    qsize,
                                )
                            {
                                mx = opt.get_unchecked(kc - i) + 1;
                            }
                        }
                        *opt.get_unchecked_mut(kc) = mx;
                        total_max = std::cmp::max(total_max, mx);
                        if kc >= lb && total_max >= lb as i32 {
                            return Some(cid);
                        }
                    }
                }
                if opt.iter().skip(lb).max().unwrap() >= &(lb as i32) {
                    return Some(cid);
                }
                None
            })
            // .filter(|cid| self.count_filter(*cid, sig_size, k, &chunks))
            .collect_vec();
        let filter_duration = filter_time.elapsed();
        // let candidates = cs.iter().cloned().collect_vec();

        Ok((candidates, index_lookup_dur, filter_duration))
    }

    /*fn count_filter(&mut self, cid: usize, sig_size: usize, k: usize, chunks: &[QSig]) -> bool {
        let start = Instant::now();
        let candidate_grams = &self.q_grams[cid].1;
        let mut candidate_gram_matches = Vec::with_capacity(candidate_grams.len());
        // let mut candidate_gram_matches = 0;

        let lb = sig_size - k;

        let (mut i, mut j) = (0, 0);
        // Since this code will always perform bound checking, even if we check it manually in while condition
        // it's faster to use UNSAFE get_unchecked.
        let mut mismatches = 0;
        while i < chunks.len() && j < candidate_grams.len() {
            // if mismatch > chunks.len() - lb {
            //     return false;
            // }
            unsafe {
                if chunks.get_unchecked(i).sig < candidate_grams.get_unchecked(j).sig {
                    i += 1;
                    mismatches += 1;
                    if mismatches > chunks.len() - lb {
                        self.cnt += start.elapsed();
                        return false;
                    }
                } else if chunks.get_unchecked(i).sig > candidate_grams.get_unchecked(j).sig {
                    j += 1;
                } else {
                    if chunks
                        .get_unchecked(i)
                        .pos
                        .abs_diff(candidate_grams.get_unchecked(j).pos)
                        <= k
                    {
                        candidate_gram_matches.push((
                            chunks.get_unchecked(i),
                            candidate_grams.get_unchecked(j).pos,
                        ));
                        i += 1;
                    }
                    j += 1;
                }
            }
        }
        self.cnt += start.elapsed();
        if candidate_gram_matches.len() < lb {
            return false;
        }
        // chunks.sort_by_key(|c| c.pos);
        let start = Instant::now();

        candidate_gram_matches.sort_by_key(|(chunk, _)| chunk.pos);

        // true match filter
        let omni_match = QSig {
            sig: vec![-1, -1],
            pos: usize::MAX,
        };
        candidate_gram_matches.insert(0, (&omni_match, omni_match.pos));
        let mut opt = vec![0; candidate_gram_matches.len()];

        // the first in tuple is the q-chunk of query, second is q-gram of data string
        #[inline(always)]
        fn compatible(m1: &(&QSig, usize), m2: &(&QSig, usize), n: usize) -> bool {
            // return value of compatible
            m2.0.sig[0] == -1
                || ((m1.0.pos != m2.0.pos && m1.0.sig != m2.0.sig) && m1.1 >= m2.1 + n)
        }

        unsafe {
            let mut total_max = i32::MIN;
            for k in 1..candidate_gram_matches.len() {
                let mut mx = i32::MIN;
                let mn = std::cmp::min(k, candidate_gram_matches.len() - lb + 1);
                for i in 1..=mn {
                    if compatible(
                        candidate_gram_matches.get_unchecked(k),
                        candidate_gram_matches.get_unchecked(k - i),
                        self.q,
                    ) && *opt.get_unchecked(k - i) > mx
                    {
                        mx = opt.get_unchecked(k - i) + 1;
                    }
                }
                *opt.get_unchecked_mut(k) = mx;
                total_max = std::cmp::max(total_max, mx);
                if k >= lb && total_max >= lb as i32 {
                    self.true_matches += start.elapsed();
                    return true;
                }
            }
        }
        self.true_matches += start.elapsed();
        opt.iter().skip(lb).max().unwrap() >= &(lb as i32)
    }

    fn strlen_from_qgrams(&self, grams: &[QSig]) -> usize {
        if grams.last().unwrap().sig.last().unwrap() == &Self::EMPTY_VALUE {
            return grams.len();
        }
        grams.len() + (self.q - 1)
    }*/
}
