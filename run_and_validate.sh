# #!/bin/bash
# RESULTS=resources/results/${1}/
# MT=${2:-''}
# QSIZE=$([ -z ${3} ] && echo "--qgram-size 2" || echo "--qgram-size $3")

# if [[ $# -eq 2 && -n "$2" ]]; then
#     QSIZE=$(echo "--qgram-size $2")
#     MT=''
# fi;

# mkdir -p ${RESULTS}
# # time ./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket lower-bound --output ${RESULTS}/${2}-candidates.csv ${2} ${TAU}
# # ./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket validate --candidates-path ${RESULTS}/${2}-candidates.csv --results-path resources/workloads/distances-${1}.csv ${TAU} 2>/dev/null

# RUSTFLAGS="-C target-cpu=native" cargo run --release -- -d resources/workloads/${1}_sorted.bracket lower-bound -q resources/workloads/${1}_query_sample.csv --output ${RESULTS} ${QSIZE} ${MT}
# # cargo run --release -- -d resources/workloads/${1}_sorted.bracket lower-bound -q resources/workloads/queries-${1}.csv --output ${RESULTS} ${QSIZE} ${MT}

# cargo build  --release
# DS=${1}
# MTD=${2:-'new-sed'}

# mkdir -p resources/results/${DS}/${MTD}
# ./target/release/tree-statistics --quiet -d $WS/traditional/${DS}_sorted.bracket lower-bound --query-file $WS/divided/${DS}/queries.csv --output resources/results/${DS}/${MTD}/ sed-struct

# ./resources/query_validate_3 $WS/traditional/${DS}_sorted.bracket $WS/divided/${DS}/queries.csv resources/results/${DS}/${MTD}/SEDStruct_candidates.csv > resources/results/${DS}/${MTD}/verified.csv

WS=resources/workloads
for DS in "sentiment" "ptb" "dblp" "rna" "python" "treefam"; do
    echo "Running for dataset: $DS";
    # ./target/release/tree-statistics --quiet -d $WS/traditional/${DS}_sorted.bracket lower-bound --query-file $WS/divided/${DS}/queries.csv --output $WS/divided/${DS}/ sed-struct >> $WS/divided/${DS}/query_times.csv;
    # mv $WS/divided/${DS}/SEDStruct_candidates.csv $WS/divided/${DS}/Sedstruct_candidates.csv;
    # python3 candidates_to_trees.py $WS/traditional/${DS}_sorted.bracket $WS/divided/${DS}/Sedstruct_candidates.csv $WS/divided/${DS}/Sedstruct_trees.bracket;
    # mkdir -p $WS/divided/${DS}/Sedstruct-stats;
    ./target/release/tree-statistics --quiet -d $WS/divided/${DS}/Sedstruct_trees.bracket statistics --hists $WS/divided/${DS}/Sedstruct-stats | tail -n 2 > $WS/divided/${DS}/Sedstruct-stats/collection.csv;
done;
