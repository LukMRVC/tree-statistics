#!/usr/bin/env python3

import click
import os
import polars as pl
from typing import IO


@click.command()
@click.option("--input", "-i", type=click.File(), required=True, help="Input file path")
def cli(input: IO):
    content = input.readlines()
    sizes = [l.count("{") for l in content]
    df = pl.DataFrame({"size": sizes})
    agg = df.group_by("size").agg(pl.len().alias("count")).sort("size")

    filename = os.path.basename(input.name)
    print("Processing file:", filename)

    # divide the dataset into 3 groups, small, medium, and large so that
    # each group has roughly the same number of trees
    total_count = agg["count"].sum()
    cumulative = agg.with_columns(
        pl.col("count").cum_sum().alias("cumulative_fraction")
    )
    agg = cumulative.with_columns(
        pl.when(pl.col("cumulative_fraction") <= pl.lit(1 / 3))
        .then(pl.lit("small"))
        .when(pl.col("cumulative_fraction") <= pl.lit(2 / 3))
        .then(pl.lit("medium"))
        .otherwise(pl.lit("large"))
        .alias("group")
    )

    group_change_sizes = agg.filter((pl.col("group") != pl.col("group").shift(1)))[
        "size"
    ]
    [small_break, medium_break] = group_change_sizes.to_list()
    small, medium, big = [], [], []
    for t, s in zip(content, sizes):
        if s <= small_break:
            small.append(t)
        elif s <= medium_break:
            medium.append(t)
        else:
            big.append(t)

    output_dir = os.path.join("resources", "workloads", "divided", filename)
    os.makedirs(os.path.dirname(output_dir), exist_ok=True)
    with open(os.path.join(output_dir, "small"), "w") as f:
        f.writelines(small)
    with open(os.path.join(output_dir, "medium"), "w") as f:
        f.writelines(medium)
    with open(os.path.join(output_dir, "large"), "w") as f:
        f.writelines(big)
    print("File processed and saved to:", output_dir)


if __name__ == "__main__":
    cli()
