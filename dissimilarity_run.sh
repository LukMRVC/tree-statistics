#!/bin/bash

DS=$1
LBLS=$2
BASE="${3:-50}"
echo $BASE;
P=resources/workloads/generated/base-${BASE}/${DS}
mkdir -p $P

python3 tree_generator.py -T 1000 -D $LBLS -S 0.5 -M 95,105 -B 50 > $P/trees.bracket
./target/release/tree-statistics -d $P/trees.bracket statistics | tail -n 2 > $P/collection.csv
python3  dissimilarity_query_gen.py -F $P/trees.bracket

./target/release/tree-statistics --quiet -d $P/trees.bracket lower-bound -q $P/dissimilarity_query.csv --output $P | tail -n 18 > $P/query_times.txt
./resources/query_validate $P/trees.bracket $P/dissimilarity_query.csv $P/Structural_candidates.csv topdiff > $P/verified.txt