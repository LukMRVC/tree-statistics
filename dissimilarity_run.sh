#!/bin/bash

DS=$1
LBLS=$2
# BASE="${3:-50}"
SIZES="40,80"
SIM="${3:-0.5}"
SELECTIVITY="${4:-1}"
echo $BASE;
P=resources/workloads/generational/generated-$(echo $SIZES | tr ',' '-')/${DS}
mkdir -p $P
echo $P;


# python3 tree_generator.py -T 1000 -D $LBLS -S 0.5 -M 95,105 -X $SIM -A $SIZES -G 70 > $P/trees.bracket && echo 'Done generating'
# ./target/release/tree-statistics -d $P/trees.bracket statistics | tail -n 2 > $P/collection.csv
# python3  dissimilarity_query_gen.py -F $P/trees.bracket

# ./target/release/tree-statistics --quiet -d $P/trees.bracket lower-bound -q $P/dissimilarity_query.csv --output $P sed | tail -n 18 > $P/query_times.txt
# cat $P/query_times.txt
# ./resources/query_validate $P/trees.bracket $P/dissimilarity_query.csv $P/Sed_candidates.csv topdiff > $P/dissimilarity-verified.txt 2> /dev/null
# ./resources/ted-ds-dist $P trees.bracket 95
./create_query_sample.py $P/distances-tjoin.txt $P/trees.bracket --target-selectivity $SELECTIVITY > $P/${SELECTIVITY}query.csv

QUERY_RESULT=$P/query-${SELECTIVITY}
mkdir -p $QUERY_RESULT

for it in {1..3}; do
    ./target/release/tree-statistics --quiet -d $P/trees.bracket lower-bound -q $P/${SELECTIVITY}query.csv --output $QUERY_RESULT | tail -n 18 > $QUERY_RESULT/query_times_${it}.txt
done
./resources/query_validate_3 $P/trees.bracket $P/${SELECTIVITY}query.csv $QUERY_RESULT/Lblint_candidates.csv $QUERY_RESULT/Sed_candidates.csv $QUERY_RESULT/Structural_candidates.csv > $QUERY_RESULT/verified-all.txt 2> /dev/null
# ./resources/query_validate $P/trees.bracket $P/1query.csv $QUERY_RESULT/Sed_candidates.csv topdiff > $QUERY_RESULT/sed-verified.txt 2> /dev/null
# ./resources/query_validate $P/trees.bracket $P/1query.csv $QUERY_RESULT/Structural_candidates.csv topdiff > $QUERY_RESULT/structural-verified.txt 2> /dev/null
