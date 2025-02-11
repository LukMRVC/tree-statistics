#!/usr/bin/env python
import click
import random
import copy
from concurrent.futures import ProcessPoolExecutor
import functools
import bisect


class TreeNode:
    """Basic tree node class."""

    def __init__(self, label, index: int, size: int = 0, parent: 'TreeNode' = None):
        self.label = label
        self.children: list[TreeNode] = []
        self.index = index
        self.size = size
        self.parent = parent

    def add_child(self, child: 'TreeNode'):
        self.children.append(child)

    def get_size(self) -> int:
        _s = len(self.children)
        for c in self.children:
            _s += c.get_size()
        return _s
    
    # get all nodes of the tree
    def get_all_nodes(self) -> list['TreeNode']:
        nodes = [self]
        for c in self.children:
            nodes.extend(c.get_all_nodes())
        return nodes

    def __repr__(self, level=0):
        # ret = "\t" * level + repr(self.label) + "\n"
        ret = r"{" + str(self.label)
        for child in self.children:
            ret += child.__repr__(level + 1)
        return ret + r"}"

    def __str__(self):
        return self.__repr__()


def generate_random_tree(size: int, labels: list[int], shape_modifier: float):
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
        child = TreeNode(random.choice(labels), i, parent=parent)
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


def generate_random_tree_from_base(
    tree: TreeNode,
    similarity: float,
    labels: list[int],
    max_edits: int = 9999999,
    current_edits: int = 0,
) -> TreeNode:
    if random.random() > similarity and current_edits < max_edits:
        # for increased randomness, we can add more operations
        random_ops = random.randint(1, 2)
        for _ in range(random_ops):
            [op] = random.choices(["label", "append"], weights=[2, 1])
            match op:
                case "label":
                    tree.label = random.choice(labels)
                case "append":
                    tree.children.append(TreeNode(random.choice(labels), -1, parent=tree))
            current_edits += 1
    for c in tree.children:
        _, ce = generate_random_tree_from_base(c, similarity, labels, max_edits, current_edits)
        current_edits += ce
    return tree, current_edits


def validate_shape_modifier(ctx, param, value):
    if not (0 < value < 1):
        raise click.BadParameter("Must be between (0, 1)")
    return value


def validate_min_max_tree_size(_, __, value):
    if "," not in value:
        raise click.BadParameter("Must contain a comma as separator! e.g. 10,50")
    try:
        mn, mx = value.split(",")
    except ValueError:
        raise click.BadParameter("Must containt exactly 2 values for min and max! e.g. 10,50...")
    return (int(mn), int(mx))


def validate_distinct_labels(_, __, value):
    if not value:
        return value
    if "," not in value:
        raise click.BadParameter("Must contain a comma as separator! e.g. 10,50")
    try:
        mn, mx = value.split(",")
    except ValueError:
        raise click.BadParameter("Must containt exactly 2 values for min and max! e.g. 10,50...")
    return (int(mn), int(mx))


def random_base_tree_generator(
    tree_count: int,
    distinct_labels: int,
    shape_modifier: float,
    min_max_tree_size: tuple[int, int],
    base_trees: None | int,
    max_edits: None | int,
    similarity: float,
    distinct_labels_per_tree: None | tuple[int, int],
):
    min_size, max_size = min_max_tree_size

    labels = [i for i in range(1, distinct_labels + 1)]

    trees: list[TreeNode] = []
    f = functools.partial(generate_random_tree, shape_modifier=shape_modifier)

    # have 1/5 of trees as "base random trees"
    base_trees = base_trees if base_trees is not None else tree_count // 10 * 2
    base_tree_sizes = sorted(random.randint(min_size, max_size) for _ in range(base_trees))

    dmin, dmax = distinct_labels_per_tree or (1, 1)
    with ProcessPoolExecutor() as p:
        for tree in p.map(
            f,
            base_tree_sizes,
            [
                (random.choices(labels, k=random.randint(dmin, dmax)) if distinct_labels_per_tree else labels)
                for _ in range(base_trees)
            ],
        ):
            trees.append(tree)

    derived = (tree_count - base_trees) // base_trees
    i = 0
    max_edits_dist = max_edits if max_edits is not None else 99999999
    for tree in trees[:base_trees]:
        for _ in range(derived):
            # i += 1
            # print(i, 'out of ', len(trees) * derived)
            dt, _ = generate_random_tree_from_base(
                copy.deepcopy(tree),
                similarity=similarity,
                labels=labels,
                max_edits=max_edits_dist,
            )
            trees.append(dt)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


def generational_random_generator(
    tree_count: int,
    generation_max_new_nodes: int,
    distinct_labels: int,
    shape_modifier: float,
    similarity: float,
    min_max_tree_size: tuple[int, int],
    distinct_labels_per_tree: None | tuple[int, int],
):
    # first generation is just a single tree
    min_size, max_size = min_max_tree_size
    labels = [i for i in range(1, distinct_labels + 1)]
    trees: list[TreeNode] = []
    f = functools.partial(generate_random_tree, shape_modifier=shape_modifier)
    # get size for first tree
    tree_size = random.randint(min_size, max_size)
    # get distinct labels for first tree
    dmin, dmax = distinct_labels_per_tree or (1, len(labels) - 1)
    max_label_idx = random.randint(dmin, dmax)
    labels_to_use = labels[:max_label_idx]
    # generate first tree
    trees.append(f(tree_size, labels_to_use))
    # generate next generations
    prev_generation_size = 1
    while len(trees) < tree_count:
        if len(labels_to_use) < len(labels):
            labels_to_use.extend(labels[max_label_idx : max_label_idx + generation_max_new_nodes])
            max_label_idx += generation_max_new_nodes
        # next_generation_size = min(prev_generation_size * 2, generation_max_new_nodes)
        next_generation_size = min(prev_generation_size * 2, 2)
        # generate new generation from previous generation
        current_trees_len = len(trees)

        for gen_base_tree in trees[current_trees_len - prev_generation_size : current_trees_len]:
            for _ in range(next_generation_size):
                if len(trees) == tree_count:
                    break
                # generate new tree from base tree
                new_tree, _ = generate_random_tree_from_base(
                    copy.deepcopy(gen_base_tree),
                    similarity=similarity,
                    labels=labels,
                    max_edits=9999999,
                )
                
                new_tree: TreeNode
                new_tree_size = new_tree.get_size()
                if not (min_size > new_tree_size > max_size):
                    delete_modifications = new_tree_size - random.randint(min_size, max_size)
                    all_tree_nodes = new_tree.get_all_nodes()
                    all_tree_nodes.remove(new_tree)
                    for _ in range(delete_modifications):
                        # pick any random node
                        node_to_remove = None
                        while node_to_remove is None:
                            node_parent = random.choice(all_tree_nodes)
                            if not node_parent.children:
                                node_parent = node_parent.parent
                            node_to_remove = random.choice(node_parent.children)
                        
                        for c in node_to_remove.children:
                            c.parent = node_parent
                        
                        node_parent.children.extend(node_to_remove.children)
                        node_parent.children.remove(node_to_remove)
                        all_tree_nodes.remove(node_to_remove)

                trees.append(new_tree)
        prev_generation_size = next_generation_size

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


@click.command()
@click.option(
    "-T",
    "--tree_count",
    required=True,
    type=int,
    help="Tree count in resulting dataset",
)
@click.option(
    "-D",
    "--distinct_labels",
    required=True,
    type=int,
    help="Number of distinct labels in collection",
)
@click.option(
    "-S",
    "--shape_modifier",
    required=True,
    type=float,
    help="Shape for each tree >0.5 for more width, <0.5 for mode depth",
    callback=validate_shape_modifier,
)
@click.option(
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-B",
    "--base_trees",
    required=False,
    type=int,
    help="Number of base trees from which to permute",
)
@click.option(
    "-E",
    "--max_edits",
    required=False,
    type=int,
    help="Number of maximum edits in each tree",
)
@click.option(
    "-X",
    "--similarity",
    required=False,
    type=float,
    help="Number of maximum edits in each tree",
    default=0.5,
)
@click.option(
    "-A",
    "--distinct_labels_per_tree",
    required=False,
    type=str,
    help="Distinct labels range per tree, delimited by comma",
    callback=validate_distinct_labels,
)
# add parameter for maximum new nodes to add in each new generation of trees
@click.option(
    "-G",
    "--max_new_nodes",
    required=False,
    type=int,
    help="Maximum number of new nodes to add in each new generation of trees",
)
def cli(
    tree_count: int,
    distinct_labels: int,
    shape_modifier: float,
    min_max_tree_size: tuple[int, int],
    base_trees: None | int,
    max_edits: None | int,
    similarity: None | float = 0.5,
    distinct_labels_per_tree: None | tuple[int, int] = None,
    max_new_nodes: None | int = None,
):
    # print(min_max_tree_size, shape_modifier, tree_count)
    # V kazde nove generaci primichame nove labely

    if not max_new_nodes:
        random_base_tree_generator(
            tree_count,
            distinct_labels,
            shape_modifier,
            min_max_tree_size,
            base_trees,
            max_edits,
            similarity,
            distinct_labels_per_tree,
        )
    else:
        generational_random_generator(
            tree_count,
            max_new_nodes,
            distinct_labels,
            shape_modifier,
            similarity,
            min_max_tree_size,
            distinct_labels_per_tree,
        )


if __name__ == "__main__":
    cli()
