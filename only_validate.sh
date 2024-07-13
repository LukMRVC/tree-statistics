TAU=${3:-10}

echo "Threshold is ${TAU}"
cargo run -q --release -- -d resources/workloads/${1}_sorted.bracket validate --candidates-path ./struct${2}-${1}-candidates.csv --results-path resources/workloads/distances-${1}.csv ${TAU} 2>/dev/null
