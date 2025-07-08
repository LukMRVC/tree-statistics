#!/usr/bin/env python3

import polars as pl
import sys
from math import ceil
from os.path import join
from concurrent.futures import ProcessPoolExecutor
import click


@click.command()
@click.argument("orig_data_path", type=click.Path(exists=True))
@click.argument("dist_path", type=click.Path(exists=True))
# accept a path to another existing file, that is just plain text file
@click.argument("picked_path", type=click.Path(exists=True))
# accept an int argument that is minimum threshold value
@click.option("--min-threshold", type=int, default=1)
# accept an int argument that is maximum threshold value
@click.option("--max-threshold", type=int, default=9999)
# option for target selectivity of a query
@click.option("--target-selectivity", type=float, default=1)
def cli(
    orig_data_path,
    dist_path,
    picked_path,
    min_threshold,
    max_threshold,
    target_selectivity,
):
    # read the orig data file and get the lines into array
    with open(orig_data_path) as f:
        orig_ds = {l.strip(): idx for idx, l in enumerate(f)}
    # read the data file and get the lines into array
    with open(picked_path) as f:
        picked_ds = {orig_ds[l.strip()]: l.strip() for l in f}

    df = pl.read_csv(
        dist_path,
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

    # get target percent based on target selectivity
    target_percent = one_percent * target_selectivity

    # get half a percent from the distinct values
    half_percent = ceil(one_percent / 2)
    # iterate from 1 to max_dist, for each iteration, get rows where dist is less than or equal to the current iteration
    # group by T1 and count the number of rows in each group
    # filter out rows where the count is greater than 1.5 percent and less than 0.5 percent of the total distinct values
    query_set = dict()
    for tau in range(min_threshold, query_tau_end + 1):
        # print(i)
        g = df.filter(df["dist"] <= tau).group_by("T1").agg(cnt=pl.len())
        g = g.filter(
            (g["cnt"] >= target_percent - half_percent)
            & (g["cnt"] < (target_percent + half_percent))
        )
        # print(g.head())
        # iterate through the rows in the group and add set T1 value to the query_set if not already in the set
        dictdf = g.to_dict(as_series=True)
        for t1 in dictdf["T1"]:
            if t1 not in query_set and t1 in picked_ds:
                # print(t1)
                query_set[t1] = tau
            if len(query_set.keys()) >= 300:
                # break outer loop
                break  # break inner loop
        else:
            continue
        break

    sorted_query_set = sorted(query_set.items(), key=lambda x: x[0])

    # read the data file and get the lines into array
    for t1, dist in sorted_query_set:
        print(f"{dist};{picked_ds[t1]}")
    # print(f"Number of queries: {len(query_set)}")
    # print(f"Max value in dist column: {max_dist}")


if __name__ == "__main__":
    cli()
