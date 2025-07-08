import polars as pl
import RNA

# df = pl.read_parquet('hf://datasets/multimolecule/rnastralign/train.parquet')


# df.select('secondary_structure').write_csv('./secondary_structure.csv')
# print(df.head())


with open('./secondary_structure.csv') as f:
    with open('./secondary_structure.bracket', 'w') as f_out:
        for line in f:
            # structure is RNA secondary structure
            structure = line.strip()
            # Convert the structure into a HIT tree string
            hit_tree = RNA.db_to_tree_string(structure, RNA.STRUCTURE_TREE_HIT)
            # Convert the structure into tree bracket notation
            hit_tree: str
            hit_tree = hit_tree[::-1].translate(str.maketrans("()","}{"))
            f_out.write(hit_tree + '\n')


    # TODO: Convert the structure to a HIT notation