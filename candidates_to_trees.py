import polars as pl
import math
import click
from os.path import join
from typing import IO
from concurrent.futures import ProcessPoolExecutor

BASEPATH = 'resources/workloads/traditional/'

def parity_check(line: str) -> bool:
    """
    Check if the number of '{' and '}' in the line are equal.

    :param line: The line to check.
    :return: True if the number of '{' and '}' are equal, False otherwise.
    """
    return (line.count('{') - line.count('\\{')) == (line.count('}') - line.count('\\}'))


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
    all_lines = dataset.readlines()
    
    with ProcessPoolExecutor() as executor:
        # Read the dataset file in parallel
        dataset_lines = [l for l, ok in zip(all_lines, executor.map(parity_check, all_lines, chunksize=1000)) if ok]
    
    # Parallel process all lines if count('{') - count('}') is ok
    print(f"Read {len(all_lines)} but only {len(dataset_lines)} are valid")
    
    trees = {tid: t for tid, t in enumerate(dataset_lines)}
    # Read the candidates file
    candidate_ids = set([int(line.strip().split(',')[1]) for line in candidates])

    # Write the result to the output file
    for cid in candidate_ids:
        tree = trees[cid]
        output.write(f"{tree}")


if __name__ == '__main__':
    cli()
