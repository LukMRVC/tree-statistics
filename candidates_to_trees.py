import polars as pl
import math
import click
from os.path import join
from typing import IO

BASEPATH = 'resources/workloads/traditional/'




@click.command()
@click.argument('dataset', type=click.File('r'))
@click.argument('candidates', type=click.File('r'))
@click.argument('output', type=click.File('w'))
def cli(dataset: IO, candidates: IO, output: IO):
    """
    Convert candidates to trees.

    :param dataset: The dataset file.
    :param candidates: The candidates file.
    :param output: The output file.
    """
    # Read the dataset file
    trees = {tid: t for tid, t in enumerate(dataset)}
    # Read the candidates file
    candidate_ids = set([int(line.strip().split(',')[1]) for line in candidates])

    # Write the result to the output file
    for cid in candidate_ids:
        tree = trees[cid]
        output.write(f"{tree}")


if __name__ == '__main__':
    cli()
