#!/usr/bin/env python
import click
import random
from concurrent.futures import ProcessPoolExecutor
import functools
import bisect

class TreeNode:
    """Basic tree node class."""
    def __init__(self, label, index: int, size: int=0):
        self.label = label
        self.children = []
        self.index = index
        self.size = size

    def add_child(self, child):
        self.children.append(child)

    def __repr__(self, level=0):
        # ret = "\t" * level + repr(self.label) + "\n"
        ret = r"{" + str(self.label)
        for child in self.children:
            ret += child.__repr__(level + 1)
        return ret + r"}"

    def __str__(self):
        return self.__repr__()



def generate_random_tree(size: int, shape_modifier: float, labels: list[int]):
    """Generate a random tree."""
    root = TreeNode(random.choice(labels), 0, size)
    nodes = [root]
    weights = [1 - shape_modifier]
    for i in range(size - 1):
        parent = random.choices(
            nodes, 
            weights=weights,
        )[0]
        weights[parent.index] = shape_modifier
        child = TreeNode(random.choice(labels), i)
        parent.add_child(child)
        nodes.append(child)
        weights.append(1 - shape_modifier)
    return root


def validate_shape_modifier(ctx, param, value):
    if not (0 < value < 1):
        raise click.BadParameter('Must be between (0, 1)')
    return value
    
def validate_min_max_tree_size(_, __, value):
    if ',' not in value:
        raise click.BadParameter('Must contain a comma as separator! e.g. 10,50')
    try:
        mn, mx = value.split(',')
    except ValueError:
        raise click.BadParameter('Must containt exactly 2 values for min and max! e.g. 10,50...')
    return (int(mn), int(mx))

@click.command()
@click.option('-T', '--tree_count', required=True, type=int, help='Tree count in resulting dataset')
@click.option('-D', '--distinct_labels', required=True, type=int,
              help='Number of distinct labels in collection')
@click.option('-S', '--shape_modifier', required=True, type=float,
                help='Shape for each tree >0.5 for more width, <0.5 for mode depth',
                callback=validate_shape_modifier)
@click.option('-M', '--min_max_tree_size', required=True, type=str,
              help='Min and max tree size, delimited by comma',
              callback=validate_min_max_tree_size)
def cli(tree_count: int, distinct_labels:int, shape_modifier: float, min_max_tree_size: tuple[int, int]):
    print(min_max_tree_size, shape_modifier, tree_count)
    min_size, max_size = min_max_tree_size

    labels = [i for i in range(1, distinct_labels + 1)]

    trees = []
    f = functools.partial(generate_random_tree, shape_modifier=shape_modifier, labels=labels)

    with ProcessPoolExecutor() as p:
        for tree in p.map(f, (random.randint(min_size, max_size) for _ in range(tree_count))):
            bisect.insort(trees, tree, key=lambda x: x.size)
            # trees.append(tree)
            # print(tree)


    # for _ in range(tree_count):
    #     size = random.randint(min_size, max_size)
    #     tree = generate_random_tree(size, shape_modifier, labels)
    #     trees.append(tree)
    
    for idx, tree in enumerate(trees):
        print(f"Tree({tree.size}) {idx + 1}:\n{tree}\n")

if __name__ == '__main__':
    cli()