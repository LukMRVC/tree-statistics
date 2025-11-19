#!/usr/bin/env python
import click
import random
import copy
import numpy as np
from concurrent.futures import ProcessPoolExecutor
import functools
import bisect
import math


class TreeNode:
    """Basic tree node class."""

    def __init__(self, label, index: int, size: int = 0, parent: "TreeNode" = None):
        self.label = label
        self.children: list[TreeNode] = []
        self.index = index
        self.size = size
        self.parent = parent

    def add_child(self, child: "TreeNode"):
        self.children.append(child)

    def get_size(self) -> int:
        _s = len(self.children)
        for c in self.children:
            _s += c.get_size()
        return _s

    # get all nodes of the tree
    def get_all_nodes(self) -> list["TreeNode"]:
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
        # random_ops = random.randint(1, 2)
        random_ops = 1
        for _ in range(random_ops):
            [op] = random.choices(["label", "append"], weights=[2, 1])
            match op:
                case "label":
                    tree.label = random.choice(labels)
                case "append":
                    tree.children.append(
                        TreeNode(random.choice(labels), -1, parent=tree)
                    )
            current_edits += 1
    for c in tree.children:
        _, ce = generate_random_tree_from_base(
            c, similarity, labels, max_edits, current_edits
        )
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
        raise click.BadParameter(
            "Must containt exactly 2 values for min and max! e.g. 10,50..."
        )
    return (int(mn), int(mx))


def validate_distinct_labels(_, __, value):
    if not value:
        return value
    if "," not in value:
        raise click.BadParameter("Must contain a comma as separator! e.g. 10,50")
    try:
        mn, mx = value.split(",")
    except ValueError:
        raise click.BadParameter(
            "Must containt exactly 2 values for min and max! e.g. 10,50..."
        )
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
    base_tree_sizes = sorted(
        random.randint(min_size, max_size) for _ in range(base_trees)
    )

    dmin, dmax = distinct_labels_per_tree or (1, 1)
    with ProcessPoolExecutor() as p:
        for tree in p.map(
            f,
            base_tree_sizes,
            [
                (
                    random.choices(labels, k=random.randint(dmin, dmax))
                    if distinct_labels_per_tree
                    else labels
                )
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
    # f = functools.partial(generate_random_tree, shape_modifier=shape_modifier)
    # f = functools.partial(unbalanced_tree, shape_modifier=shape_modifier)
    # get size for first tree
    tree_size = random.randint(min_size, max_size)
    # get distinct labels for first tree
    dmin, dmax = distinct_labels_per_tree or (1, len(labels) - 1)
    max_label_idx = random.randint(dmin, dmax) + 500
    labels_to_use = labels[:max_label_idx]
    # generate first tree
    # unbalanced tree probability - 1 is for balanced, 0 for unbalanced
    trees.append(generate_unbalanced_trees(1, distinct_labels, min_max_tree_size, 0)[0])
    # generate next generations
    prev_generation_size = 1
    while len(trees) < tree_count:
        if len(labels_to_use) < len(labels):
            labels_to_use.extend(
                labels[max_label_idx : max_label_idx + generation_max_new_nodes]
            )
            max_label_idx += generation_max_new_nodes
        # next_generation_size = min(prev_generation_size * 2, generation_max_new_nodes)
        next_generation_size = min(
            max(prev_generation_size * 2, 2), generation_max_new_nodes
        )
        # generate new generation from previous generation
        current_trees_len = len(trees)

        for gen_base_tree in trees[
            current_trees_len - prev_generation_size : current_trees_len
        ]:
            for _ in range(next_generation_size):
                if len(trees) == tree_count:
                    break
                # generate new tree from base tree
                new_tree = copy.deepcopy(gen_base_tree)
                # get random number of edits to apply
                max_edits = len([x for x in range(5) if random.random() < similarity])
                tree_nodes = new_tree.get_all_nodes()
                tree_nodes.remove(new_tree)  # cannot edit the root
                for _ in range(max_edits):
                    [op] = random.choices(["label", "append"], weights=[3, 1])
                    node = random.choice(tree_nodes)
                    # [op] = random.choices(["label"], weights=[2])
                    match op:
                        case "label":
                            node.label = random.choice(labels)
                        case "append":
                            # if append, I append whole subtree of size 3 and remove some subtree
                            node.add_child(
                                TreeNode(random.choice(labels), -1, parent=node)
                            )

                new_tree_size = new_tree.get_size()
                if not (min_size > new_tree_size > max_size):
                    delete_modifications = new_tree_size - random.randint(
                        min_size, max_size
                    )
                    all_tree_nodes = new_tree.get_all_nodes()
                    # remove the root from the list of nodes so we don't remove it by accident
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


# --- CLI REFACTOR ---
import sys


@click.group()
def cli():
    """Tree generator CLI with subcommands for schema, random base, and generational trees."""
    pass


@cli.command("unbalanced-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-P",
    "--probability",
    required=False,
    type=float,
    default=0,
    help="Probability of children having the another children",
)
def unbalanced_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    probability: float,
):
    """Generate trees with a fixed schema that is unbalanced. Control imbalance with shape_modifier."""
    trees = generate_unbalanced_trees(
        tree_count, distinct_labels, min_max_tree_size, probability
    )
    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


def generate_unbalanced_trees(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    probability: float,
):
    labels = [i for i in range(1, distinct_labels + 1)]
    min_size, max_size = min_max_tree_size
    trees = []

    root_label = random.choice(labels)

    # I need to pull a random number from exponential distribution where a small sample of numbers
    # is more likely to be smaller than the mean, and a large sample of numbers is
    # more likely to be larger than the mean.

    # create a labels list so that each node with that given label has children with only labels from that list
    # pull numbers from exponential distribution

    adj_sizes = 2 + np.random.exponential(scale=3, size=len(labels)).astype(int)
    # median_adj_size = np.median(adj_sizes)
    # mean_adj_size = np.mean(adj_sizes)
    # print(f"Median of adj_sizes: {median_adj_size}")
    # print(f"Mean of adj_sizes: {mean_adj_size}")

    labels_adj = {
        lbl: random.choices(labels, k=adj_sizes[i]) for i, lbl in enumerate(labels)
    }

    for _ in range(tree_count):
        size = random.randint(min_size, max_size)
        root = TreeNode(root_label, 0, size=size)
        trees.append(root)
        tree_nodes_created = 1

        current = root
        level = 1
        num_children = 3

        # tree_label_set = set([root.label])
        while tree_nodes_created < size:
            # Number of children is determined by shape_modifier
            # Lower shape_modifier -> fewer children (more unbalanced/deep)
            # Higher shape_modifier -> more children (less unbalanced/wider)
            children: list[TreeNode] = list()
            for _ in range(num_children):
                # if same_labels is 4, then for 4 levels I want the middle child to have the same label
                label = random.choice(labels_adj[current.label])

                # while label in tree_label_set:
                # label = (
                #     random.choice(labels)
                # )
                # tree_label_set.add(label)

                c = TreeNode(label, 0, parent=current)
                children.append(c)
            for child in children:
                current.add_child(child)

            for c in children[:-1]:
                if tree_nodes_created > size:
                    break
                if random.random() < probability:
                    for _ in range(num_children):
                        c.add_child(TreeNode(random.choice(labels), 0, parent=c))
                        tree_nodes_created += 1

            tree_nodes_created += len(children)
            # Pick the last child to continue the chain (makes it unbalanced)
            current = children[-1]
            level += 1
    return sorted(trees, key=lambda t: t.get_size())


@cli.command("balanced-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
def balanced_tree(
    tree_count: int, distinct_labels: int, min_max_tree_size: tuple[int, int]
):
    """Generate trees with a fixed schema that is balanced."""
    labels = [i for i in range(1, distinct_labels + 1)]
    min_size, max_size = min_max_tree_size
    trees = []

    def build_balanced_tree(size, label_idx=0, parent=None):
        if size <= 0:
            return None
        node = TreeNode(labels[label_idx % len(labels)], 0, size=size, parent=parent)
        if size == 1:
            return node
        # Calculate number of children (N) for perfect balance
        # For simplicity, use 2 children (binary tree), but you can change N as needed
        N = 3
        child_size = (size - 1) // N
        remainder = (size - 1) % N
        for i in range(N):
            cs = child_size + (1 if i < remainder else 0)
            if cs > 0:
                child = build_balanced_tree(cs, label_idx + i + 1, parent=node)
                node.add_child(child)
        return node

    for _ in range(tree_count):
        size = random.randint(min_size, max_size)
        tree = build_balanced_tree(size)
        trees.append(tree)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


@cli.command("binary-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-O",
    "--option",
    required=True,
    type=str,
    help="type: LeftBinary, ZigZag, FullBinary",
)
def binary_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    option: str,
):
    """Generate binary trees with a fixed schema."""
    """Generate trees with a fixed schema that is balanced."""
    labels = [i for i in range(1, distinct_labels + 1)]
    min_size, max_size = min_max_tree_size
    trees = []

    def generate_full_binary_tree(depth, parent=None):
        """Recursively generate a full binary tree of given depth."""
        if depth == 0:
            return None
        node = TreeNode(random.choice(labels), 0, parent=parent)
        if depth > 1:
            left_child = generate_full_binary_tree(depth - 1, node)
            right_child = generate_full_binary_tree(depth - 1, node)
            node.add_child(left_child)
            node.add_child(right_child)
        return node

    for _ in range(tree_count):
        size = random.randint(min_size, max_size)
        root = TreeNode(random.choice(labels), 0)
        tree_size = 0

        if option == "FullBinary":
            max_depth = math.ceil(math.log2(size + 1))
            trees.append(generate_full_binary_tree(max_depth))
            continue

        current = root
        while tree_size + 1 < size:
            # Choose a random node to add a child to
            c1 = TreeNode(random.choice(labels), 0, parent=current)
            c2 = TreeNode(random.choice(labels), 0, parent=current)
            current.add_child(c1)
            current.add_child(c2)
            tree_size += 2
            match option:
                case "LeftBinary":
                    current = c1  # Always go left
                case "ZigZag":
                    # alternate between left and right
                    current = c1 if tree_size % 4 == 0 else c2
        trees.append(root)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


@cli.command("random-base-tree")
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
    help="Shape for each tree >0.5 for more width, <0.5 for more depth",
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
    help="Similarity for edits",
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
def random_base_tree(
    tree_count,
    distinct_labels,
    shape_modifier,
    min_max_tree_size,
    base_trees,
    max_edits,
    similarity,
    distinct_labels_per_tree,
):
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


@cli.command("generational-tree")
@click.option(
    "-T",
    "--tree_count",
    required=True,
    type=int,
    help="Tree count in resulting dataset",
)
@click.option(
    "-G",
    "--max_new_nodes",
    required=True,
    type=int,
    help="Maximum number of new nodes to add in each new generation of trees",
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
    help="Shape for each tree >0.5 for more width, <0.5 for more depth",
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
    "-X",
    "--similarity",
    required=False,
    type=float,
    help="Similarity for edits",
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
def generational_tree(
    tree_count,
    max_new_nodes,
    distinct_labels,
    shape_modifier,
    min_max_tree_size,
    similarity,
    distinct_labels_per_tree,
):
    generational_random_generator(
        tree_count,
        max_new_nodes,
        distinct_labels,
        shape_modifier,
        similarity,
        min_max_tree_size,
        distinct_labels_per_tree,
    )


def generate_fanout_tree(
    size: int, labels: list[int], fanout: float, labels_adj: dict[int, list[int]]
) -> TreeNode:
    """
    Generates a random tree of a given size.
    The fanout parameter controls the shape of the tree.
    - fanout close to 1.0 results in a bushy tree (high fanout).
    - fanout close to 0.0 results in a skinny, deep tree (low fanout).
    """
    if not (0 < fanout < 1):
        raise ValueError("Fanout must be between 0 and 1.")

    root = TreeNode(labels[0], 0, size)
    nodes = [root]
    # Weights for parent selection. High fanout favors existing nodes, creating a bushy tree.
    # Low fanout favors newly added nodes, creating a deep/skinny tree.
    weights = [fanout]
    for i in range(1, size):
        parent = random.choices(nodes, weights=weights, k=1)[0]
        label = random.choice(labels_adj[parent.label])
        # label = random.choice(labels)

        child = TreeNode(label, i, parent=parent)
        parent.add_child(child)

        nodes.append(child)
        weights.append(1 - fanout)  # New nodes get a weight of (1-fanout)

    return root


@cli.command("fanout-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-F",
    "--fanout",
    required=True,
    type=click.FloatRange(0.0, 1.0, clamp=True),
    help="Fanout factor for tree generation. Close to 1.0 is bushy, close to 0.0 is skinny.",
)
def fanout_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    fanout: float,
):
    """Generate random trees based on a fanout parameter."""
    labels = list(range(1, distinct_labels + 1))
    min_size, max_size = min_max_tree_size
    trees = []
    adj_sizes = 4 + np.random.exponential(scale=4, size=len(labels)).astype(int)

    labels_adj = {
        lbl: random.choices(labels, k=adj_sizes[i]) for i, lbl in enumerate(labels)
    }


    # generate a base tree to use as a template
    base_tree = generate_fanout_tree(
        random.randint(min_size, max_size), labels, fanout, labels_adj
    )
    print("Base tree generated", file=sys.stderr)

    # for _ in range(tree_count // 2):
    #     trees.append(generate_fanout_tree(random.randint(min_size, max_size), labels, fanout, labels_adj))
    # for _ in range(tree_count - 1):
    #     size = random.randint(min_size, max_size)
    #     tree = generate_fanout_tree(size, labels, fanout, labels_adj)
    #     trees.append(tree)

    # TODO: Adjust changes to preserve fanout and sizes

    # TODO: Do a better set of edit operations
    # 1. Delete a random leaf
    # 2. Insert a new leaf at a random position
    # 3. Siblings swap - a node that has at least 2 siblings - swap 2 siblings
    # 4. Subtree Prune and Re-attach - Randomly select a small subtree and reattach it at a different position

    while len(trees) < tree_count:
        # for base_tree in trees[:tree_count // 2]:
        # copy the base tree and make some random edits
        new_tree = copy.deepcopy(base_tree)
        # get the number of edits to make
        num_edits = random.randint(2
                                   , 5)
        all_nodes = new_tree.get_all_nodes()
        # remove root node from the list of nodes to edit
        all_nodes.remove(new_tree)
        for _ in range(num_edits):
            if not all_nodes:
                break

            tries = 0
            # To preserve fanout, we primarily change labels.
            # For structural changes, we swap nodes or subtrees.
            # [op] = random.choices(["label", "swap"], weights=[2, 1])

            # Lower fanout means more delete/insert leaf operations
            leaf_weight = max(1, int(8 * (1 - fanout)))
            other_weight = max(1, int(4 * fanout))

            op = random.choice(
                ["sibling-swap", "subtree-move"],
                # ["delete-leaf", "insert-leaf", "sibling-swap", "subtree-move"],
                # weights=[leaf_weight, leaf_weight, other_weight, other_weight],
            )

            match op:
                case "label":
                    node_to_edit = random.choice(all_nodes)
                    node_to_edit.label = random.choice(
                        [l for l in labels if l != node_to_edit.label]
                    )
                case "delete-leaf":
                    # get random leaf node to delete
                    node_to_edit = random.choice(
                        [n for n in all_nodes if not n.children]
                    )
                    # remove the node from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # remove the node from the list of all nodes
                    all_nodes.remove(node_to_edit)
                    if new_tree.get_size() < min_size:
                        # re-add the node if we went below min size
                        # select random leaf, to which parent we will reattach the a new node
                        rnd_leaf = random.choice(
                            [n for n in all_nodes if not n.children]
                        )
                        new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                        rnd_leaf.add_child(new_node)
                        all_nodes.append(new_node)
                case "insert-leaf":
                    # select random leaf, to which parent we will reattach the a new node
                    rnd_leaf = random.choice([n for n in all_nodes if not n.children])
                    new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                    rnd_leaf.add_child(new_node)
                    all_nodes.append(new_node)
                    if new_tree.get_size() > max_size:
                        # remove a random leaf if we went above max size
                        leaf_to_remove = random.choice(
                            [n for n in all_nodes if not n.children and n != new_node]
                        )
                        leaf_to_remove.parent.children.remove(leaf_to_remove)
                        all_nodes.remove(leaf_to_remove)
                case "sibling-swap":
                    node_to_edit = random.choice(all_nodes)
                    while len(node_to_edit.children) < 2:
                        node_to_edit = random.choice(all_nodes)
                        tries += 1
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue

                    c1, c2 = random.sample(node_to_edit.children, 2)
                    # swap positions in node_to_edit's children list
                    idx1 = node_to_edit.children.index(c1)
                    idx2 = node_to_edit.children.index(c2)
                    node_to_edit.children[idx1], node_to_edit.children[idx2] = c2, c1

                case "subtree-move":
                    # TODO: Ensure we don't create cycles or invalid structures - the trees are actually reduced
                    node_to_edit = random.choice(all_nodes)
                    all_subtree_nodes = node_to_edit.get_all_nodes()
                    all_subtree_nodes.append(node_to_edit.parent)
                    new_possible_parents = [
                        n for n in all_nodes if n not in all_subtree_nodes
                    ]
                    # move a subtree to a different position
                    while len(node_to_edit.children) < 1 or not new_possible_parents:
                        node_to_edit = random.choice(all_nodes)
                        all_subtree_nodes = node_to_edit.get_all_nodes()
                        all_subtree_nodes.append(node_to_edit.parent)
                        new_possible_parents = [
                            n for n in all_nodes if n not in all_subtree_nodes
                        ]
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue
                    # the current node_to_edit is the one to move
                    # remove the current subtree from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # select a new parent for the subtree from parent and siblings nodes

                    new_parent = random.choice(new_possible_parents)
                    new_parent.add_child(node_to_edit)
                    node_to_edit.parent = new_parent
                case "swap":
                    # Swap two random non-root nodes
                    if len(all_nodes) < 2:
                        continue

                    node1 = node_to_edit

                    nodes_for_swap = all_nodes[:]
                    nodes_for_swap.remove(node1)

                    # Avoid swapping with a direct ancestor or descendant
                    ancestors = set()
                    p = node1.parent
                    while p:
                        ancestors.add(p)
                        if p in nodes_for_swap:
                            nodes_for_swap.remove(p)
                        p = p.parent

                    descendants = set(node1.get_all_nodes())
                    descendants.remove(node1)
                    for d in descendants:
                        if d in nodes_for_swap:
                            nodes_for_swap.remove(d)

                    if not nodes_for_swap:
                        continue

                    node2 = random.choice(nodes_for_swap)

                    # Swap parents
                    p1, p2 = node1.parent, node2.parent
                    if p1 and node1 in p1.children and p2 and node2 in p2.children:
                        idx1 = p1.children.index(node1)
                        idx2 = p2.children.index(node2)
                        p1.children[idx1], p2.children[idx2] = node2, node1
                        node1.parent, node2.parent = p2, p1

        trees.append(new_tree)

    # for _ in range(tree_count):
    #     trees.append(tree)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)



def generate_average_fanout_tree(
    size: int,
    labels: list[int],
    avg_fanout: float,
    labels_adj: dict[int, list[int]],
    epsilon: float = 0.5,
) -> TreeNode:
    """
    Generate a tree with a target average fanout (average number of children per internal node).

    Args:
      size: Total number of nodes in the tree
      labels: List of possible node labels
      avg_fanout: Target average fanout (children per internal node)
      labels_adj: Dictionary mapping labels to their possible child labels
      epsilon: Acceptable deviation from target average fanout

    Returns:
      TreeNode: Root of the generated tree
    """
    if size <= 0:
        raise ValueError("Size must be positive")
    if avg_fanout <= 0:
        raise ValueError("Average fanout must be positive")

    root = TreeNode(labels[0], 0, size)
    nodes = [root]
    nodes_to_expand = [root]
    created_count = 1

    while created_count < size and nodes_to_expand:
        # Select parent to expand
        parent = nodes_to_expand.pop(0)

        # Calculate how many children to add
        remaining = size - created_count
        remaining_parents = len(nodes_to_expand) + 1

        # Determine number of children based on avg_fanout with some randomness
        if remaining_parents > 0:
            target_children = max(
                1,
                int(
                    avg_fanout
                    + random.uniform(-epsilon * avg_fanout, epsilon * avg_fanout)
                ),
            )
        else:
            target_children = remaining

        num_children = min(target_children, remaining)

        # Add children
        for _ in range(num_children):
            if created_count >= size:
                break

            label = random.choice(labels_adj.get(parent.label, labels))
            child = TreeNode(label, created_count, parent=parent)
            parent.add_child(child)
            nodes.append(child)
            nodes_to_expand.append(child)
            created_count += 1

    return root


def calculate_average_fanout(tree: TreeNode) -> float:
    """Calculate the average fanout of a tree (avg children per internal node)."""
    all_nodes = tree.get_all_nodes()
    internal_nodes = [n for n in all_nodes if n.children]

    if not internal_nodes:
        return 0.0

    total_children = sum(len(n.children) for n in internal_nodes)
    return total_children / len(internal_nodes)


def calculate_node_depth(node: TreeNode) -> int:
    """Calculate the depth of a node from the root (root has depth 1)."""
    depth = 1
    current = node
    while current.parent:
        depth += 1
        current = current.parent
    return depth


def calculate_weighted_branching_factor(tree: TreeNode) -> float:
    """
    Calculate the weighted branching factor of a tree.
    Formula: sum(degree / depth²) / num_internal_nodes for all internal nodes.
    Normalized by the number of internal nodes.
    Shallow nodes contribute more weight than deep nodes.
    """
    all_nodes = tree.get_all_nodes()
    internal_nodes = [n for n in all_nodes if n.children]

    if not internal_nodes:
        return 0.0

    total_weighted_branching = 0.0
    for node in internal_nodes:
        degree = len(node.children)
        depth = calculate_node_depth(node)
        total_weighted_branching += degree / (depth ** 2)

    return total_weighted_branching / len(internal_nodes)


def generate_weighted_branching_tree(
    size: int,
    labels: list[int],
    target_weighted_branching: float,
    labels_adj: dict[int, list[int]],
    epsilon: float = 0.5,
    max_attempts: int = 100,
) -> TreeNode:
    """
    Generate a tree with a target weighted branching factor.
    
    The weighted branching factor is: sum(degree / depth²) / num_internal_nodes.
    This metric is normalized by the number of internal nodes and prioritizes 
    wide, shallow trees over deep, narrow ones.
    
    Typical range: 0.1 - 3.0+
    - Low values (0.1-0.3): Deep, chain-like trees
    - Medium values (0.4-0.6): Balanced trees
    - High values (0.7-1.5): Wide, shallow trees
    - Very high values (1.5+): Extremely wide, very shallow trees
    
    Args:
      size: Total number of nodes in the tree
      labels: List of possible node labels
      target_weighted_branching: Target weighted branching factor (typically 0.1-3.0)
      labels_adj: Dictionary mapping labels to their possible child labels
      epsilon: Acceptable deviation from target weighted branching factor
      max_attempts: Maximum iterations to try to achieve target
      
    Returns:
      TreeNode: Root of the generated tree
    """
    if size <= 0:
        raise ValueError("Size must be positive")
    if target_weighted_branching <= 0:
        raise ValueError("Target weighted branching factor must be positive")

    root = TreeNode(labels[0], 0, size)
    nodes = [root]
    nodes_to_expand = [root]
    created_count = 1

    # Determine generation strategy based on target (typical range 0.1-3.0+)
    # High target means wide+shallow, low target means deep+linear
    
    # Build initial tree structure using BFS with controlled branching
    while created_count < size and nodes_to_expand:
        parent = nodes_to_expand.pop(0)
        parent_depth = calculate_node_depth(parent)
        
        remaining = size - created_count
        
        # Calculate desired number of children for this node
        # The contribution of this node to the metric will be: degree / depth²
        # We want the average contribution across all internal nodes to equal target
        
        if parent_depth == 1:
            # Root node: its contribution is degree/1² = degree
            # Root heavily influences the final metric
            if target_weighted_branching >= 1.5:
                # Very high target (1.5+): extremely wide shallow tree
                # Root needs very many children
                num_children = min(remaining, random.randint(8, 15))
            elif target_weighted_branching >= 0.7:
                # High target (0.7-1.5): very wide shallow tree
                # Root needs many children to boost metric
                num_children = min(remaining, random.randint(5, 8))
            elif target_weighted_branching >= 0.5:
                # Medium-high target (0.5-0.7): moderately wide
                num_children = min(remaining, random.randint(3, 5))
            elif target_weighted_branching >= 0.3:
                # Medium target (0.3-0.5): balanced
                num_children = min(remaining, random.randint(2, 4))
            else:
                # Low target (0.1-0.3): deep narrow tree
                num_children = min(remaining, random.randint(1, 3))
        else:
            # For deeper nodes, their contribution decreases with depth²
            # Adjust branching based on depth and target
            
            if target_weighted_branching >= 1.5:
                # Very high target: extremely shallow tree (depth ≤ 2)
                if parent_depth >= 2:
                    num_children = 0  # Stop expanding at depth 2
                else:
                    num_children = min(remaining, random.randint(4, 8))
                    
            elif target_weighted_branching >= 0.7:
                # High target: keep tree very shallow (depth ≤ 2-3)
                if parent_depth >= 3:
                    num_children = 0  # Stop expanding at depth 3
                elif parent_depth == 2:
                    num_children = min(remaining, random.randint(1, 4))
                else:
                    num_children = min(remaining, random.randint(2, 5))
                    
            elif target_weighted_branching >= 0.5:
                # Medium-high: moderate depth (depth ≤ 4-5)
                if parent_depth >= 5:
                    num_children = 0
                elif parent_depth >= 3:
                    num_children = min(remaining, random.randint(0, 2))
                else:
                    num_children = min(remaining, random.randint(1, 3))
                    
            elif target_weighted_branching >= 0.3:
                # Medium: balanced branching
                base = max(1, int(3.0 / parent_depth))
                num_children = min(remaining, random.randint(max(0, base - 1), base + 1))
                
            else:
                # Low target: create deep chains
                # Mostly continue with single child (chain)
                if random.random() < 0.6:
                    num_children = min(remaining, 1)
                else:
                    num_children = min(remaining, random.randint(1, 2))
        
        # Add children
        for _ in range(num_children):
            if created_count >= size:
                break

            label = random.choice(labels_adj.get(parent.label, labels))
            child = TreeNode(label, created_count, parent=parent)
            parent.add_child(child)
            nodes.append(child)
            nodes_to_expand.append(child)
            created_count += 1

    # Fine-tune the tree to match target weighted branching factor
    for attempt in range(max_attempts):
        current_weighted_branching = calculate_weighted_branching_factor(root)
        
        if abs(current_weighted_branching - target_weighted_branching) <= epsilon:
            break

        all_nodes = root.get_all_nodes()
        internal_nodes = [n for n in all_nodes if n.children and n != root]
        
        if not internal_nodes:
            break

        diff = current_weighted_branching - target_weighted_branching
        
        if diff < 0:
            # Current metric too low - need to increase weighted branching
            # Strategy: Move children from deep nodes to shallow nodes
            # This increases the metric because shallow nodes have higher weight (1/depth²)
            
            # Find shallow nodes (depth 1-2) and deep nodes (depth 3+)
            shallow_nodes = [n for n in internal_nodes if calculate_node_depth(n) <= 2]
            deep_nodes = [n for n in internal_nodes if calculate_node_depth(n) >= 3 and len(n.children) > 1]
            
            if shallow_nodes and deep_nodes:
                # Move a child from a deep node to a shallow node
                donor_node = random.choice(deep_nodes)
                target_node = random.choice(shallow_nodes)
                
                child_to_move = random.choice(donor_node.children)
                donor_node.children.remove(child_to_move)
                target_node.add_child(child_to_move)
                child_to_move.parent = target_node
            elif shallow_nodes:
                # If no deep nodes, try to add depth to make room for redistribution
                # Pick a leaf at shallow depth and make it internal
                leaves_at_shallow = [n for n in all_nodes 
                                    if not n.children 
                                    and n.parent 
                                    and calculate_node_depth(n) <= 2]
                if leaves_at_shallow and len(internal_nodes) > 0:
                    # Convert a leaf to internal by stealing a child from somewhere
                    new_parent = random.choice(leaves_at_shallow)
                    donor_candidates = [n for n in internal_nodes if len(n.children) > 2]
                    if donor_candidates:
                        donor = random.choice(donor_candidates)
                        child = random.choice(donor.children)
                        donor.children.remove(child)
                        new_parent.add_child(child)
                        child.parent = new_parent
        else:
            # Current metric too high - need to decrease weighted branching
            # Strategy: Move children from shallow nodes to deeper nodes
            # This decreases the metric because deep nodes have lower weight
            
            shallow_nodes = [n for n in internal_nodes if calculate_node_depth(n) <= 2 and len(n.children) > 1]
            deep_nodes = [n for n in internal_nodes if calculate_node_depth(n) >= 3]
            
            if shallow_nodes and deep_nodes:
                # Move a child from shallow to deep
                donor_node = random.choice(shallow_nodes)
                target_node = random.choice(deep_nodes)
                
                child_to_move = random.choice(donor_node.children)
                donor_node.children.remove(child_to_move)
                target_node.add_child(child_to_move)
                child_to_move.parent = target_node
            elif shallow_nodes:
                # If no deep nodes exist, create depth by extending a chain
                # Take a child from shallow node and move it to one of its own children
                donor_node = random.choice(shallow_nodes)
                if len(donor_node.children) > 1:
                    children_with_children = [c for c in donor_node.children if c.children]
                    if children_with_children:
                        child_to_extend = random.choice(children_with_children)
                        sibling = random.choice([c for c in donor_node.children if c != child_to_extend])
                        donor_node.children.remove(sibling)
                        child_to_extend.add_child(sibling)
                        sibling.parent = child_to_extend

    return root


@cli.command("average-fanout-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-F",
    "--avg_fanout",
    required=True,
    type=float,
    help="Target average fanout (average number of children per internal node)",
)
@click.option(
    "-E",
    "--epsilon",
    required=False,
    type=float,
    default=0.5,
    help="Acceptable deviation from target average fanout.",
)
def average_fanout_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    avg_fanout: float,
    epsilon: float,
):
    """Generate random trees based on average fanout parameter."""
    labels = list(range(1, distinct_labels + 1))
    min_size, max_size = min_max_tree_size
    trees = []

    adj_sizes = 4 + np.random.exponential(scale=4, size=len(labels)).astype(int)
    labels_adj = {
        lbl: random.choices(labels, k=adj_sizes[i]) for i, lbl in enumerate(labels)
    }

    # Generate base tree
    base_tree = generate_average_fanout_tree(
        random.randint(min_size, max_size), labels, avg_fanout, labels_adj, epsilon
    )

    # Regenerate base tree if it doesn't meet fanout target
    attempts = 0
    while (
        abs(calculate_average_fanout(base_tree) - avg_fanout) > epsilon
        and attempts < 10
    ):
        base_tree = generate_average_fanout_tree(
            random.randint(min_size, max_size), labels, avg_fanout, labels_adj, epsilon
        )
        attempts += 1

    trees.append(base_tree)

    # Generate variations
    while len(trees) < tree_count:
        new_tree = copy.deepcopy(base_tree)
        num_edits = random.randint(3, max_size // 5)
        all_nodes = new_tree.get_all_nodes()
        all_nodes.remove(new_tree)  # Don't edit root

        print(len(trees), file=sys.stderr)

        for _ in range(num_edits):

            op = random.choice(
                ["sibling-swap", "subtree-move", "insert-leaf", "delete-leaf"]
            )

            if op == "label":
                node = random.choice(all_nodes)
                node.label = random.choice([l for l in labels if l != node.label])
            elif op == "insert-leaf":
                # select random leaf, to which parent we will reattach the a new node
                leaves = [n for n in all_nodes if not n.children]
                if not leaves:
                    num_edits += 1
                    continue

                rnd_leaf = random.choice(leaves)
                new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)

                rnd_leaf.add_child(new_node)
                all_nodes.append(new_node)
                if new_tree.get_size() > max_size:
                    # remove a random leaf if we went above max size
                    removable_leaves = [
                        n for n in all_nodes if not n.children and n != new_node
                    ]
                    if not removable_leaves:
                        num_edits += 1
                        continue
                    leaf_to_remove = random.choice(removable_leaves)
                    leaf_to_remove.parent.children.remove(leaf_to_remove)
                    all_nodes.remove(leaf_to_remove)
            elif op == "delete-leaf":
                # get random leaf node to delete
                node_to_edit = random.choice([n for n in all_nodes if not n.children])
                # remove the node from its parent
                node_to_edit.parent.children.remove(node_to_edit)
                # remove the node from the list of all nodes
                all_nodes.remove(node_to_edit)
                if new_tree.get_size() < min_size:
                    # re-add the node if we went below min size
                    # select random leaf, to which parent we will reattach the a new node
                    rnd_leaf = random.choice([n for n in all_nodes if not n.children])
                    new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                    rnd_leaf.add_child(new_node)
                    all_nodes.append(new_node)
            elif op == "sibling-swap":
                nodes_with_siblings = [
                    n for n in all_nodes if n.parent and len(n.parent.children) >= 2
                ]
                if nodes_with_siblings:
                    node = random.choice(nodes_with_siblings)
                    siblings = node.parent.children
                    if len(siblings) >= 2:
                        idx1, idx2 = random.sample(range(len(siblings)), 2)
                        siblings[idx1], siblings[idx2] = siblings[idx2], siblings[idx1]
            elif op == "subtree-move":
                node_to_move = random.choice(all_nodes)
                all_subtree_nodes = node_to_move.get_all_nodes()
                all_subtree_nodes.append(node_to_move.parent)
                new_possible_parents = [
                    n for n in all_nodes if n not in all_subtree_nodes
                ]
                if new_possible_parents:
                    # Remove from old parent
                    node_to_move.parent.children.remove(node_to_move)
                    # Attach to new parent
                    new_parent = random.choice(new_possible_parents)
                    new_parent.add_child(node_to_move)
                    node_to_move.parent = new_parent

        # Check if average fanout is still within acceptable range
        new_avg_fanout = calculate_average_fanout(new_tree)
        if abs(new_avg_fanout - avg_fanout) <= epsilon:
            trees.append(new_tree)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)




@cli.command("fanout-percentage-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-F",
    "--fanout",
    required=True,
    type=click.FloatRange(0.0, 1.0, clamp=True),
    help="Fanout factor as percentage of leaf to total nodes. Close to 1.0 is bushy, close to 0.0 is skinny.",
)
@click.option(
    "-E",
    "--epsilon",
    required=False,
    type=float,
    default=0.1,
    help="Acceptable deviation from target fanout ratio.",
)
def fanout_percentage_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    fanout: float,
    epsilon: float,
):
    """Generate random trees based on a fanout parameter."""
    labels = list(range(1, distinct_labels + 1))
    min_size, max_size = min_max_tree_size
    trees = []
    adj_sizes = 4 + np.random.exponential(scale=4, size=len(labels)).astype(int)

    labels_adj = {
        lbl: random.choices(labels, k=adj_sizes[i]) for i, lbl in enumerate(labels)
    }

    def get_leaf_to_total_ratio(tree: TreeNode) -> float:
        all_nodes = tree.get_all_nodes()
        leaf_nodes = [n for n in all_nodes if not n.children]
        return len(leaf_nodes) / len(all_nodes)

    # generate a base tree to use as a template
    base_tree = generate_fanout_tree(
        random.randint(min_size, max_size), labels, fanout, labels_adj
    )

    while fanout + epsilon < get_leaf_to_total_ratio(base_tree) < fanout - epsilon:
        base_tree = generate_fanout_tree(
            random.randint(min_size, max_size), labels, fanout, labels_adj
        )

    # for _ in range(tree_count // 2):
    #     trees.append(generate_fanout_tree(random.randint(min_size, max_size), labels, fanout, labels_adj))
    # for _ in range(tree_count - 1):
    #     size = random.randint(min_size, max_size)
    #     tree = generate_fanout_tree(size, labels, fanout, labels_adj)
    #     trees.append(tree)

    # TODO: Adjust changes to preserve fanout and sizes

    # TODO: Do a better set of edit operations
    # 1. Delete a random leaf
    # 2. Insert a new leaf at a random position
    # 3. Siblings swap - a node that has at least 2 siblings - swap 2 siblings
    # 4. Subtree Prune and Re-attach - Randomly select a small subtree and reattach it at a different position

    while len(trees) < tree_count:
        # for base_tree in trees[:tree_count // 2]:
        # copy the base tree and make some random edits
        new_tree = copy.deepcopy(base_tree)
        # get the number of edits to make
        num_edits = random.randint(2, max_size // 5)
        all_nodes = new_tree.get_all_nodes()
        # remove root node from the list of nodes to edit
        all_nodes.remove(new_tree)
        for _ in range(num_edits):
            if not all_nodes:
                break

            tries = 0
            # To preserve fanout, we primarily change labels.
            # For structural changes, we swap nodes or subtrees.
            # [op] = random.choices(["label", "swap"], weights=[2, 1])

            # Lower fanout means more delete/insert leaf operations
            leaf_weight = max(1, int(8 * (1 - fanout)))
            other_weight = max(1, int(4 * fanout))

            op = random.choices(
                ["delete-leaf", "insert-leaf", "sibling-swap", "subtree-move"],
                weights=[leaf_weight, leaf_weight, other_weight, other_weight],
            )

            match op:
                case "label":
                    node_to_edit = random.choice(all_nodes)
                    node_to_edit.label = random.choice(
                        [l for l in labels if l != node_to_edit.label]
                    )
                case "delete-leaf":
                    # get random leaf node to delete
                    node_to_edit = random.choice(
                        [n for n in all_nodes if not n.children]
                    )
                    # remove the node from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # remove the node from the list of all nodes
                    all_nodes.remove(node_to_edit)
                    # if new_tree.get_size() < min_size:
                    # re-add the node if we went below min size
                    # select random leaf, to which parent we will reattach the a new node
                    rnd_leaf = random.choice([n for n in all_nodes if not n.children])
                    new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                    rnd_leaf.add_child(new_node)
                    all_nodes.append(new_node)
                case "insert-leaf":
                    # select random leaf, to which parent we will reattach the a new node
                    rnd_leaf = random.choice([n for n in all_nodes if not n.children])
                    new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                    rnd_leaf.add_child(new_node)
                    all_nodes.append(new_node)
                    if new_tree.get_size() > max_size:
                        # remove a random leaf if we went above max size
                        leaf_to_remove = random.choice(
                            [n for n in all_nodes if not n.children and n != new_node]
                        )
                        leaf_to_remove.parent.children.remove(leaf_to_remove)
                        all_nodes.remove(leaf_to_remove)
                case "sibling-swap":
                    node_to_edit = random.choice(all_nodes)
                    while len(node_to_edit.children) < 2:
                        node_to_edit = random.choice(all_nodes)
                        tries += 1
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue

                    c1, c2 = random.sample(node_to_edit.children, 2)
                    # swap positions in node_to_edit's children list
                    idx1 = node_to_edit.children.index(c1)
                    idx2 = node_to_edit.children.index(c2)
                    node_to_edit.children[idx1], node_to_edit.children[idx2] = c2, c1

                case "subtree-move":
                    # TODO: Ensure we don't create cycles or invalid structures - the trees are actually reduced
                    node_to_edit = random.choice(all_nodes)
                    all_subtree_nodes = node_to_edit.get_all_nodes()
                    all_subtree_nodes.append(node_to_edit.parent)
                    new_possible_parents = [
                        n for n in all_nodes if n not in all_subtree_nodes
                    ]
                    # move a subtree to a different position
                    while len(node_to_edit.children) < 1 or not new_possible_parents:
                        node_to_edit = random.choice(all_nodes)
                        all_subtree_nodes = node_to_edit.get_all_nodes()
                        all_subtree_nodes.append(node_to_edit.parent)
                        new_possible_parents = [
                            n for n in all_nodes if n not in all_subtree_nodes
                        ]
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue
                    # the current node_to_edit is the one to move
                    # remove the current subtree from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # select a new parent for the subtree from parent and siblings nodes

                    new_parent = random.choice(new_possible_parents)
                    new_parent.add_child(node_to_edit)
                    node_to_edit.parent = new_parent
                case "swap":
                    # Swap two random non-root nodes
                    if len(all_nodes) < 2:
                        continue

                    node1 = node_to_edit

                    nodes_for_swap = all_nodes[:]
                    nodes_for_swap.remove(node1)

                    # Avoid swapping with a direct ancestor or descendant
                    ancestors = set()
                    p = node1.parent
                    while p:
                        ancestors.add(p)
                        if p in nodes_for_swap:
                            nodes_for_swap.remove(p)
                        p = p.parent

                    descendants = set(node1.get_all_nodes())
                    descendants.remove(node1)
                    for d in descendants:
                        if d in nodes_for_swap:
                            nodes_for_swap.remove(d)

                    if not nodes_for_swap:
                        continue

                    node2 = random.choice(nodes_for_swap)

                    # Swap parents
                    p1, p2 = node1.parent, node2.parent
                    if p1 and node1 in p1.children and p2 and node2 in p2.children:
                        idx1 = p1.children.index(node1)
                        idx2 = p2.children.index(node2)
                        p1.children[idx1], p2.children[idx2] = node2, node1
                        node1.parent, node2.parent = p2, p1

        while fanout + epsilon < get_leaf_to_total_ratio(new_tree):
            # the number of leaves is too we need to reduce leaf count
            rnd_leaf = random.choice([n for n in all_nodes if not n.children])
            attach_leaf = random.choice(
                [n for n in all_nodes if n != rnd_leaf and not n.children]
            )
            rnd_leaf.parent.children.remove(rnd_leaf)
            rnd_leaf.parent = attach_leaf
            attach_leaf.add_child(rnd_leaf)

        while get_leaf_to_total_ratio(new_tree) < fanout - epsilon:
            # the number of leaves is too few we need to increase leaf count
            # by not inserting any new nodes but by change of structure
            for i in range(0, 15):
                nodes_seq = [
                    n
                    for n in all_nodes
                    if not n.children
                    and len(n.parent.children) >= i
                    and len(n.parent.children) < i + 1
                    and n.parent.parent
                ]
                if not nodes_seq:
                    continue
                random_leaf = random.choice(nodes_seq)
                if random_leaf:
                    break

            old_parent = random_leaf.parent
            # I have a leaf that has a parent and a grandparent so I can make the parent a leaf
            # by reattaching all its children to the grandparent
            new_parent = random_leaf.parent.parent
            # remove all leaf parent's children from old parent and reattach to grandparent
            for c in old_parent.children:
                new_parent.add_child(c)
                c.parent = new_parent
            old_parent.children = []

        trees.append(new_tree)

    # for _ in range(tree_count):
    #     trees.append(tree)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


@cli.command("weighted-branching-tree")
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
    "-M",
    "--min_max_tree_size",
    required=True,
    type=str,
    help="Min and max tree size, delimited by comma",
    callback=validate_min_max_tree_size,
)
@click.option(
    "-F",
    "--fanout",
    required=True,
    type=click.FloatRange(0.0, 1.0, clamp=True),
    help="Fanout factor as percentage of leaf to total nodes. Close to 1.0 is bushy, close to 0.0 is skinny.",
)
@click.option(
    "-W",
    "--target_weighted_branching",
    required=True,
    type=float,
    help="Target weighted branching factor: avg(degree/depth² for internal nodes)",
)
@click.option(
    "-E",
    "--epsilon",
    required=False,
    type=float,
    default=0.1,
    help="Acceptable deviation from target weighted branching factor.",
)
def weighted_branching_tree(
    tree_count: int,
    distinct_labels: int,
    min_max_tree_size: tuple[int, int],
    fanout: float,
    target_weighted_branching: float,
    epsilon: float,
):
    """Same as fanout generation, but it checks for weighted branching factor first."""
    labels = list(range(1, distinct_labels + 1))
    min_size, max_size = min_max_tree_size
    trees = []
    # adj_sizes = 4 + np.random.exponential(scale=4, size=len(labels)).astype(int)

    # labels_adj = {
    #     lbl: random.choices(labels, k=adj_sizes[i]) for i, lbl in enumerate(labels)
    # }

    labels_adj = {
        lbl: labels for lbl in labels
    }


    # generate a base tree to use as a template
    base_tree = generate_fanout_tree(
        random.randint(min_size, max_size), labels, fanout, labels_adj
    )
    while abs(calculate_weighted_branching_factor(base_tree) - target_weighted_branching) > epsilon:
        base_tree = generate_fanout_tree(
            random.randint(min_size, max_size), labels, fanout, labels_adj
        )
        print("Regenerated base tree, the weighted_branching_factor was=", calculate_weighted_branching_factor(base_tree), file=sys.stderr)
        
    print("Base tree generated", file=sys.stderr)

    # for _ in range(tree_count // 2):
    #     trees.append(generate_fanout_tree(random.randint(min_size, max_size), labels, fanout, labels_adj))
    # for _ in range(tree_count - 1):
    #     size = random.randint(min_size, max_size)
    #     tree = generate_fanout_tree(size, labels, fanout, labels_adj)
    #     trees.append(tree)

    # TODO: Adjust changes to preserve fanout and sizes

    # TODO: Do a better set of edit operations
    # 1. Delete a random leaf
    # 2. Insert a new leaf at a random position
    # 3. Siblings swap - a node that has at least 2 siblings - swap 2 siblings
    # 4. Subtree Prune and Re-attach - Randomly select a small subtree and reattach it at a different position

    while len(trees) < tree_count:
        # for base_tree in trees[:tree_count // 2]:
        # copy the base tree and make some random edits
        new_tree = copy.deepcopy(base_tree)
        # get the number of edits to make
        num_edits = random.randint(4, 8)
        all_nodes = new_tree.get_all_nodes()
        # remove root node from the list of nodes to edit
        all_nodes.remove(new_tree)
        for _ in range(num_edits):
            if not all_nodes:
                break

            tries = 0
            # To preserve fanout, we primarily change labels.
            # For structural changes, we swap nodes or subtrees.
            # [op] = random.choices(["label", "swap"], weights=[2, 1])

            # Lower fanout means more delete/insert leaf operations
            leaf_weight = max(1, int(8 * (1 - fanout)))
            other_weight = max(1, int(4 * fanout))

            op = random.choice(
                ["sibling-swap", "subtree-move"],
                # ["delete-leaf", "insert-leaf", "sibling-swap", "subtree-move"],
                # weights=[leaf_weight, leaf_weight, other_weight, other_weight],
            )

            match op:
                case "label":
                    node_to_edit = random.choice(all_nodes)
                    node_to_edit.label = random.choice(
                        [l for l in labels if l != node_to_edit.label]
                    )
                case "delete-leaf":
                    # get random leaf node to delete
                    node_to_edit = random.choice(
                        [n for n in all_nodes if not n.children]
                    )
                    # remove the node from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # remove the node from the list of all nodes
                    all_nodes.remove(node_to_edit)
                    if new_tree.get_size() < min_size:
                        # re-add the node if we went below min size
                        # select random leaf, to which parent we will reattach the a new node
                        rnd_leaf = random.choice(
                            [n for n in all_nodes if not n.children]
                        )
                        new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                        rnd_leaf.add_child(new_node)
                        all_nodes.append(new_node)
                case "insert-leaf":
                    # select random leaf, to which parent we will reattach the a new node
                    rnd_leaf = random.choice([n for n in all_nodes if not n.children])
                    new_node = TreeNode(random.choice(labels), -1, parent=rnd_leaf)
                    rnd_leaf.add_child(new_node)
                    all_nodes.append(new_node)
                    if new_tree.get_size() > max_size:
                        # remove a random leaf if we went above max size
                        leaf_to_remove = random.choice(
                            [n for n in all_nodes if not n.children and n != new_node]
                        )
                        leaf_to_remove.parent.children.remove(leaf_to_remove)
                        all_nodes.remove(leaf_to_remove)
                case "sibling-swap":
                    node_to_edit = random.choice(all_nodes)
                    while len(node_to_edit.children) < 2:
                        node_to_edit = random.choice(all_nodes)
                        tries += 1
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue

                    c1, c2 = random.sample(node_to_edit.children, 2)
                    # swap positions in node_to_edit's children list
                    idx1 = node_to_edit.children.index(c1)
                    idx2 = node_to_edit.children.index(c2)
                    node_to_edit.children[idx1], node_to_edit.children[idx2] = c2, c1

                case "subtree-move":
                    # TODO: Ensure we don't create cycles or invalid structures - the trees are actually reduced
                    node_to_edit = random.choice(all_nodes)
                    all_subtree_nodes = node_to_edit.get_all_nodes()
                    all_subtree_nodes.append(node_to_edit.parent)
                    new_possible_parents = [
                        n for n in all_nodes if n not in all_subtree_nodes
                    ]
                    # move a subtree to a different position
                    while len(node_to_edit.children) < 1 or not new_possible_parents:
                        node_to_edit = random.choice(all_nodes)
                        all_subtree_nodes = node_to_edit.get_all_nodes()
                        all_subtree_nodes.append(node_to_edit.parent)
                        new_possible_parents = [
                            n for n in all_nodes if n not in all_subtree_nodes
                        ]
                        if tries > 10:
                            break
                    if tries > 10:
                        num_edits += 1
                        continue
                    # the current node_to_edit is the one to move
                    # remove the current subtree from its parent
                    node_to_edit.parent.children.remove(node_to_edit)
                    # select a new parent for the subtree from parent and siblings nodes

                    new_parent = random.choice(new_possible_parents)
                    new_parent.add_child(node_to_edit)
                    node_to_edit.parent = new_parent
                case "swap":
                    # Swap two random non-root nodes
                    if len(all_nodes) < 2:
                        continue

                    node1 = node_to_edit

                    nodes_for_swap = all_nodes[:]
                    nodes_for_swap.remove(node1)

                    # Avoid swapping with a direct ancestor or descendant
                    ancestors = set()
                    p = node1.parent
                    while p:
                        ancestors.add(p)
                        if p in nodes_for_swap:
                            nodes_for_swap.remove(p)
                        p = p.parent

                    descendants = set(node1.get_all_nodes())
                    descendants.remove(node1)
                    for d in descendants:
                        if d in nodes_for_swap:
                            nodes_for_swap.remove(d)

                    if not nodes_for_swap:
                        continue

                    node2 = random.choice(nodes_for_swap)

                    # Swap parents
                    p1, p2 = node1.parent, node2.parent
                    if p1 and node1 in p1.children and p2 and node2 in p2.children:
                        idx1 = p1.children.index(node1)
                        idx2 = p2.children.index(node2)
                        p1.children[idx1], p2.children[idx2] = node2, node1
                        node1.parent, node2.parent = p2, p1

        
        if abs(calculate_weighted_branching_factor(new_tree) - target_weighted_branching) <= epsilon:
          trees.append(new_tree)

    # for _ in range(tree_count):
    #     trees.append(tree)

    for tree in sorted(trees, key=lambda t: t.get_size()):
        print(tree)


if __name__ == "__main__":
    cli()
