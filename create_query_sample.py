#!/usr/bin/env python3

import polars as pl
import sys
from math import ceil
from os.path import join
from concurrent.futures import ProcessPoolExecutor
import click


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

# for qs in setup:
#     get_queries(*qs)


@click.command()
@click.argument("csv_path", type=click.Path(exists=True))
# accept a path to another existing file, that is just plain text file
@click.argument("data_path", type=click.Path(exists=True))
# accept an int argument that is minimum threshold value
@click.option("--min-threshold", type=int, default=1)
# accept an int argument that is maximum threshold value
@click.option("--max-threshold", type=int, default=9999)
def cli(csv_path, data_path, min_threshold, max_threshold):
    df = pl.read_csv(
        csv_path,
        has_header=False,
        schema={"T1": pl.Int32(), "T2": pl.Int32(), "dist": pl.Int32()},
    )
    max_dist = df["dist"].max()

    query_tau_end = min(max_dist, max_threshold)

    df = df.sort(["T1", "T2"])
    # filter out values where T1 and T2 are equal
    df = df.filter(df["T1"] != df["T2"])
    # count the number of distinct values in column T1
    distinct_t1 = df["T1"].n_unique()
    # get 1 percent from the distinct values
    one_percent = ceil(distinct_t1 / 100)
    # get half a percent from the distinct values
    half_percent = ceil(one_percent / 2)
    # iterate from 1 to max_dist, for each iteration, get rows where dist is less than or equal to the current iteration
    # group by T1 and count the number of rows in each group
    # filter out rows where the count is greater than 1.5 percent and less than 0.5 percent of the total distinct values
    query_set = dict()
    for i in range(min_threshold, query_tau_end + 1):
        # print(i)
        g = df.filter(df["dist"] <= i).group_by("T1").agg(cnt=pl.len())
        g = g.filter(
            (g["cnt"] >= half_percent) & (g["cnt"] < (one_percent + half_percent))
        )
        # print(g.head())
        # iterate through the rows in the group and add set T1 value to the query_set if not already in the set
        dictdf = g.to_dict(as_series=True)
        for t1 in dictdf["T1"]:
            if t1 not in query_set:
                # print(t1)
                query_set[t1] = i
            if len(query_set.keys()) >= 300:
                # break outer loop
                break  # break inner loop
        else:
            continue
        break

    sorted_query_set = sorted(query_set.items(), key=lambda x: x[0])

    # read the data file and get the lines into array
    with open(data_path) as f:
        lines = [l.strip() for l in f]

    for t1, dist in sorted_query_set:
        print(f"{dist};{lines[t1]}")
    # print(f"Number of queries: {len(query_set)}")
    # print(f"Max value in dist column: {max_dist}")


if __name__ == "__main__":
    cli()


def query_selectivity(
    query_results: pl.DataFrame,
    data: pl.DataFrame,
) -> float:
    """
    Calculate the selectivity of the query results
    """
    df = query_results.group_by("T1").agg(cnt=pl.len())
    # get the number of rows in the data
    data_count = data.shape[0]

    # now divide the cnt by the data_count
    # to get the selectivity
    df["selectivity"] = df["cnt"] / data_count

    # calculate the selectivity
    return df["selectivity"].mean()
