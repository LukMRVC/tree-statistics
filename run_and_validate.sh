TAU=${3:-10}

echo "Threshold is ${TAU}"

TDS=${1}
MTD=${2}

RESULTS=resources/results/${TDS}/${TAU}



mkdir -p ${RESULTS}
time ./target/release/tree-statistics -d resources/workloads/${TDS}_sorted.bracket lower-bound --output ${RESULTS}/${MTD}-candidates.csv ${MTD} ${TAU}
./target/release/tree-statistics -d resources/workloads/${TDS}_sorted.bracket validate --candidates-path ${RESULTS}/${MTD}-candidates.csv --results-path resources/workloads/distances-${TDS}.csv ${TAU} 2>/dev/null
