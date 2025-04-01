import polars as pl

# Pick from a group of most frequent trees by tree_size
datasets_most_frequest_sizes = dict(
    bolzano=(14,16),
    sentiment=(37,39),
    # take 2 for DBLP - 25 and 27
    dblp=(37,39),
    ptb=(70,72),
    rna=(12,12),
    # take 2 for rna - 23 through 25
    # rna=(23, 25),
    swissprot=(261, 265),
    treefam=(174, 179),
    python=(49,51),
)

queries = 100
for dataset, (min_size, max_size) in datasets_most_frequest_sizes.items():
    print(f"dataset: {dataset}")
    # read the dataset
    with open(f"resources/workloads/{dataset}_sorted.bracket") as f:
        # trees: list[tuple[str, int]] = [(t, tid) for tid, t in enumerate(f) if min_size <= t.count('{') <= max_size]
        trees: list[int] = []
        for tid, t in enumerate(f):
            # strip the tree
            t = t.strip()

            ts = t.count('{') - t.count('\{')
            # check if the tree is within the size range
            if min_size <= ts <= max_size:
                trees.append(tid)

            if ts > max_size:
                break
            

    # trees = [(t, ts, tid) for t, ts, tid in trees if min_size <= ts <= max_size]
    # make trees a polars dataframe
    # trees = pl.DataFrame(trees, schema=["tree", "tree_size"])
    # pick the most frequent trees
    # trees = trees.group_by("tree_size").agg(pl.len().alias("cnt")).sort("cnt", descending=True)
    print(len(trees))

    # Now I have tree_ids of the most frequent trees by their tree size
    # Now pick 100 random trees as queries by having 1% selectivity
    qs = dict()
    # read distances as polars dataframe
    df = pl.read_csv(f"resources/workloads/distances-{dataset}.csv", has_header=False)
    
