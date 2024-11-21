import polars as pl
import sys
from math import ceil
from os.path import join
from concurrent.futures import ProcessPoolExecutor


def get_queries(dataset: str, min_k: int, max_k: int, Q: int, overrides: dict = dict()):
    distances_path = join("resources/workloads", f"distances-{dataset}.csv")

    with open(join("resources/workloads", f"{dataset}_sorted.bracket")) as f:
        lines = [l.strip() for l in f]

    size_1percent = len(lines) // 100
    MIN_RESULTS = overrides.get("min_r", None) or size_1percent // 2
    MAX_RESULTS = overrides.get("max_r", None) or size_1percent * 3
    max_queries = overrides.get("max_queries", 200)

    sig_sizes = [t.count("{") // Q for t in lines]

    df = pl.read_csv(
        distances_path,
        has_header=False,
        schema={"T1": pl.Int32(), "T2": pl.Int32(), "K": pl.Int32()},
    )
    # print(df.glimpse())

    mx = df["K"].max()
    print("Maximum threshold", mx)

    tree_ids = pl.concat(
        [df.select("T1"), df.select("T2").rename({"T2": "T1"})], how="vertical"
    ).unique()
    # print(tree_ids.glimpse())
    queries = []

    for k in range(min_k, max_k + 1):
        print("Trying K =", k)
        only_tids = [tid for tid, _ in queries]
        g = df.filter(df["K"] <= k).group_by("T1").agg(cnt=pl.len())
        g = g.filter((g["cnt"] >= MIN_RESULTS) & (g["cnt"] < MAX_RESULTS))
        # print(g.head())

        for tid in g["T1"].shuffle():
            if k < sig_sizes[tid] and tid not in only_tids:
                queries.append((tid, k))
            if len(queries) >= max_queries:
                break

        g = df.filter(df["K"] <= k).group_by("T2").agg(cnt=pl.len())
        g = g.filter((g["cnt"] >= MIN_RESULTS) & (g["cnt"] < MAX_RESULTS))
        only_tids = [tid for tid, _ in queries]
        for tid in g["T2"].shuffle():
            if k < sig_sizes[tid] and tid not in only_tids:
                queries.append((tid, k))
            if len(queries) >= max_queries:
                break
        if len(queries) >= max_queries:
            break

    print(len(queries))
    with open(join("resources/workloads", f"{dataset}_query_sample.csv"), "w") as f:
        for t, k in queries:
            f.write(f"{k};{lines[t]}\n")


setup = (
    # ("swissprot", 1, 30, 8, {"min_r": 100, "max_r": 2000}),
    # (
    #     "python",
    #     2,
    #     30,
    #     6,
    # ),
    # ("treefam", 5, 60, 4, {"min_r": 10, "max_r": 200}),
    # ("ptb", 2, 30, 2, {"min_r": 2, "max_r": 50}),
    # ("dblp", 2, 8, 3, {"min_r": 150}),
    ("sentiment", 5, 20, 2, {"min_r": 2, "max_r": 300}),
)

for qs in setup:
    get_queries(*qs)
