RESULTS=resources/results/${1}/

mkdir -p ${RESULTS}
# time ./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket lower-bound --output ${RESULTS}/${2}-candidates.csv ${2} ${TAU}
# ./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket validate --candidates-path ${RESULTS}/${2}-candidates.csv --results-path resources/workloads/distances-${1}.csv ${TAU} 2>/dev/null

cargo run --release -- -d resources/workloads/${1}_sorted.bracket lower-bound -q resources/workloads/${1}_query_sample.csv --output ${RESULTS}