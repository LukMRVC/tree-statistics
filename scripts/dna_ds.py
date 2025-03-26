import polars as pl
import RNA

# df = pl.read_parquet("hf://datasets/multimolecule/rnastralign/train.parquet")
# df.select("secondary_structure").write_csv("secondary_structure.csv")


with open("./secondary_structure.csv") as f:
    with open("./secondary_structure.bracket", "w") as g:
        for line in f:
            structure = line.strip()
            hit_tree = RNA.db_to_tree_string(structure, RNA.STRUCTURE_TREE_HIT)
            hit_tree: str
            hit_tree = hit_tree[::-1].translate(str.maketrans("()", "}{"))
            g.write(hit_tree + "\n")
