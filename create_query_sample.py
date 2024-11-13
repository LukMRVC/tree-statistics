import pandas as pd
import polars as pl
import sys
from math import ceil
from os.path import join
from concurrent.futures import ProcessPoolExecutor

dataset = sys.argv[1]
min_k = int(sys.argv[2]) if len(sys.argv) > 2 else 1
max_k = int(sys.argv[3]) if len(sys.argv) > 3 else 24
Q = 2
MAX_QUERIES = 200
MIN_RESULTS = 500
MAX_RESULTS = 3000

distances_path = join("resources/workloads", f"distances-{dataset}.csv")

with open(join("resources/workloads", f"{dataset}_sorted.bracket")) as f:
    lines = [l.strip() for l in f]


sig_sizes = [t.count("{") // Q for t in lines]

df = pl.read_csv(
    distances_path,
    has_header=False,
    schema={"T1": pl.Int32(), "T2": pl.Int32(), "K": pl.Int32()},
)
print(df.glimpse())

mx = df["K"].max()
print("Maximum threshold", mx)

tree_ids = pl.concat(
    [df.select("T1"), df.select("T2").rename({"T2": "T1"})], how="vertical"
).unique()
print(tree_ids.glimpse())

queries = []


for k in range(min_k, max_k + 1):
    print("Trying K =", k)
    only_tids = [tid for tid, _ in queries]

    g = df.filter(df["K"] <= k).group_by("T1").agg(cnt=pl.len())
    g = g.filter((g["cnt"] >= MIN_RESULTS) & (g["cnt"] < MAX_RESULTS))
    print(g.tail())
    print(g.head())

    for tid in g["T1"].shuffle():
        if k < sig_sizes[tid] and tid not in only_tids:
            queries.append((tid, k))
        if len(queries) >= MAX_QUERIES:
            break

    g = df.filter(df["K"] <= k).group_by("T2").agg(cnt=pl.len())
    g = g.filter((g["cnt"] > MIN_RESULTS) & (g["cnt"] < MAX_RESULTS))
    only_tids = [tid for tid, _ in queries]
    for tid in g["T2"].shuffle():
        if k < sig_sizes[tid] and tid not in only_tids:
            queries.append((tid, k))
        if len(queries) >= MAX_QUERIES:
            break
    if len(queries) >= MAX_QUERIES:
        break


print(len(queries))

with open(join("resources/workloads", f"{dataset}_query_sample.csv"), "w") as f:
    for t, k in queries:
        f.write(f"{k};{lines[t]}\n")
