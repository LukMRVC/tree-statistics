#!/usr/bin/env python
import click
from typing import TextIO
from os.path import join, dirname

# datasets = [
#     'lsmall',
#     'lmedium',
# ]


# for ds in datasets:
#     with open(join('resources/workloads/generated', f'{ds}.bracket')) as f:
#         with open(join('resources/workloads/generated', f'dissimilarity_{ds}_query.csv'), 'w') as qf:
#             for line in f:
#                 size = line.count("{")
#                 tau = size // 2
#                 qf.write(f'{tau};{line}')



@click.command()
@click.option('-F', '--tree_file', type=click.File('r'))
def cli(tree_file: TextIO):
    filename = tree_file.name
    dn = dirname(filename)
    with open(join(dn, f'dissimilarity_query.csv'), 'w') as qf:
        for line in tree_file:
            size = line.count("{")
            tau = size // 2
            qf.write(f'{tau};{line}')



if __name__ == '__main__':
    cli()

