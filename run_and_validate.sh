TAU=${3:-10}

echo "Threshold is ${TAU}"

time cargo run -q --release -- -d resources/workloads/${1}_sorted.bracket lower-bound --output ./struct${2}-${1}-candidates.csv structural${2} ${TAU} 2>/dev/null
cargo run -q --release -- -d resources/workloads/${1}_sorted.bracket validate --candidates-path ./struct${2}-${1}-candidates.csv --results-path resources/workloads/distances-${1}.csv ${TAU} 2>/dev/null