#!/usr/bin/env python
import click
import random
import copy
from concurrent.futures import ProcessPoolExecutor
import functools
import bisect

class TreeNode:
    """Basic tree node class."""
    def __init__(self, label, index: int, size: int=0):
        self.label = label
        self.children: list[TreeNode] = []
        self.index = index
        self.size = size

    def add_child(self, child):
        self.children.append(child)

    def get_size(self) -> int:
        _s = len(self.children)
        for c in self.children:
            _s += c.get_size()
        return _s

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

def remove_random_children(tree: TreeNode):
    if not tree.children:
        return
    child_to_delete = random.choice(tree.children)
    tree.children.extend(child_to_delete.children)
    tree.children.remove(child_to_delete)

def generate_random_tree_from_base(tree: TreeNode, similarity: float, labels: list[int], max_edits: int = 9999999, current_edits: int = 0) -> TreeNode:
    if random.random() > similarity and current_edits < max_edits:
        [op] = random.choices(['label', 'append', 'remove'], weights=[2, 1, 1])
        match op:
            case 'label': tree.label = random.choice(labels)
            case 'append': tree.children.append(TreeNode(random.choice(labels), -1))
            case 'remove': remove_random_children(tree)
        current_edits += 1
    for c in tree.children:
        _, ce = generate_random_tree_from_base(c, similarity, labels, max_edits, current_edits)
        current_edits += ce
    return tree, current_edits


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
@click.option('-B', '--base_trees', required=False, type=int, help='Number of base trees from which to permute')
@click.option('-E', '--max_edits', required=False, type=int, help='Number of maximum edits in each tree')
def cli(
    tree_count: int,
    distinct_labels:int,
    shape_modifier: float,
    min_max_tree_size: tuple[int, int],
    base_trees: None | int,
    max_edits: None | int,
    ):
    # print(min_max_tree_size, shape_modifier, tree_count)
    min_size, max_size = min_max_tree_size

    labels = [i for i in range(1, distinct_labels + 1)]

    trees: list[TreeNode] = []
    f = functools.partial(generate_random_tree, shape_modifier=shape_modifier, labels=labels)

    # have 1/5 of trees as "base random trees"
    base_trees = base_trees if base_trees is not None else tree_count // 10 * 2
    base_tree_sizes = sorted(random.randint(min_size, max_size) for _ in range(base_trees))


    with ProcessPoolExecutor() as p:
        for tree in p.map(f, base_tree_sizes):
            # bisect.insort(trees, tree, key=lambda x: x.size)
            trees.append(tree)
            # print(tree)

    derived = (tree_count - base_trees) // base_trees
    i = 0
    max_edits_dist = max_edits if max_edits is not None else 99999999
    for tree in trees[:base_trees]:
        for _ in range(derived):
            # i += 1
            # print(i, 'out of ', len(trees) * derived)
            dt, _ = generate_random_tree_from_base(copy.deepcopy(tree), similarity=0.75, labels=labels, max_edits = max_edits_dist)
            # bisect.insort(trees, dt, key=lambda x: x.size)
            trees.append(dt)


    # for _ in range(tree_count):
    #     size = random.randint(min_size, max_size)
    #     tree = generate_random_tree(size, shape_modifier, labels)
    #     trees.append(tree)
    
    # for idx, tree in enumerate(trees):
    #     print(f"Tree({tree.size}) {idx + 1}:\n{tree}\n")

    for tree in sorted(trees, key=lambda t: t.get_size()):
        # print(f"Tree({tree.size}) {idx + 1}:\n{tree}\n")
        print(tree)

if __name__ == '__main__':
    cli()
