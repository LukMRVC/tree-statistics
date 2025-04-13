import polars as pl
import math
from os.path import join

BASEPATH = "resources/workloads/traditional/"

# Pick from a group of most frequent trees by tree_size
datasets_most_frequest_sizes = dict(
    # bolzano=(14,16),
    # TODO: bigger distances are needed
    # sentiment=(37, 39),
    # take 2 for DBLP - 25 and 27
    # TODO: bigger distances are needed for this size
    # dblp=(25,25),
    # ptb=(70, 72),
    rna=(12, 12),
    # take 2 for rna - 23 through 25
    # rna=(23, 25),
    # swissprot=(261, 265),
    # treefam=(174, 179),
    # python=(49, 51),
)

for dataset, (min_size, max_size) in datasets_most_frequest_sizes.items():
    max_queries = 10 if dataset == "bolzano" else 100
    print(f"dataset: {dataset}")
    # read the dataset
    trees: dict[int, str] = dict()
    total_trees = 0
    with open(join(BASEPATH, f"{dataset}_sorted.bracket")) as f:
        # trees: list[tuple[str, int]] = [(t, tid) for tid, t in enumerate(f) if min_size <= t.count('{') <= max_size]
        for tid, t in enumerate(f):
            # strip the tree
            t = t.strip()
            total_trees += 1
            ts = t.count("{") - t.count("\\{")
            # check if the tree is within the size range
            if min_size <= ts <= max_size:
                trees[tid] = t

            if ts > max_size:
                break
        for _ in f:
            total_trees += 1

    # trees = [(t, ts, tid) for t, ts, tid in trees if min_size <= ts <= max_size]
    # make trees a polars dataframe
    # trees = pl.DataFrame(trees, schema=["tree", "tree_size"])
    # pick the most frequent trees
    # trees = trees.group_by("tree_size").agg(pl.len().alias("cnt")).sort("cnt", descending=True)
    print("Total trees", total_trees)
    single_percent_len = math.ceil(total_trees / 100)
    if single_percent_len <= 10:
        min_results = single_percent_len - 2
        max_results = single_percent_len + 2
    elif single_percent_len <= 100:
        min_results = single_percent_len - (single_percent_len / 2)
        max_results = single_percent_len + (single_percent_len / 2)
    else:
        min_results = single_percent_len - (single_percent_len / 4)
        max_results = single_percent_len + (single_percent_len / 4)
    # Now I have tree_ids of the most frequent trees by their tree size
    # Now pick 100 random trees as queries by having 1% selectivity
    qs = dict()
    # read distances as polars dataframe
    df = pl.read_csv(
        join(BASEPATH, f"distances-{dataset}.csv"),
        has_header=False,
        schema={"T1": pl.Int32(), "T2": pl.Int32(), "K": pl.Int32()},
    )
    # # sort the dataset by trees
    df = df.sort("T1", "T2")
    mx = df["K"].max()
    pickable_tids = set([tid for tid in trees.keys()])
    print("Pickable tids: ", pickable_tids)
    print("Max distance: ", mx)
    print("Min results: ", min_results, "Max results: ", max_results)

    for tau in range(1, mx + 1):
        g = df.filter((df["K"] < tau)).group_by("T1").agg(pl.len().alias("cnt"))
        g = g.filter(
            (g["cnt"] >= min_results)
            & (g["cnt"] < max_results)
            & (g["T1"].is_in(pickable_tids))
        )

        print("Tau = ", tau, " usable = ", g.shape[0], g.head(5))
        for tid in g["T1"].shuffle():
            if tid not in qs:
                qs[tid] = tau
            if len(qs) >= max_queries:
                break

        g = df.filter((df["K"] < tau)).group_by("T2").agg(pl.len().alias("cnt"))
        g = g.filter(
            (g["cnt"] >= min_results)
            & (g["cnt"] < max_results)
            & (g["T2"].is_in(pickable_tids))
        )
        print("Tau = ", tau, " usable = ", g.shape[0], g.head(5))
        for tid in g["T2"].shuffle():
            if tid not in qs:
                qs[tid] = tau
            if len(qs) >= max_queries:
                break
        if len(qs) >= max_queries:
            break

    print("Total queries for ", dataset, ": ", len(qs))
    # write the queries to a file

    print("Chosen TID", qs.keys())
    with open(f"resources/workloads/divided/{dataset}/queries.csv", "w") as f:
        for tid, tau in qs.items():
            # write the tree and tau
            f.write(f"{tau};{trees[tid]}\n")

    with open(
        f"resources/workloads/divided/{dataset}/queries_to_original_id_map.csv", "w"
    ) as f:
        f.write(f"qid;tid;{tau}\n")

        for qid, (tid, tau) in enumerate(qs.items()):
            # write the tree and tau
            f.write(f"{qid};{tid};{tau}\n")
