TAU=${3:-10}

echo "Threshold is ${TAU}"

RESULTS=resources/results/${1}/${TAU}

mkdir -p ${RESULTS}
time ./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket lower-bound --output ${RESULTS}/${2}-candidates.csv ${2} ${TAU}
./target/release/tree-statistics -d resources/workloads/${1}_sorted.bracket validate --candidates-path ${RESULTS}/${2}-candidates.csv --results-path resources/workloads/distances-${1}.csv ${TAU} 2>/dev/null
