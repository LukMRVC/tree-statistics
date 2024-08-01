#!/bin/bash

TDS=$1
TAU=$2

methods=("structural" "structural-variant" "sed" "lblint")


for m in ${methods[@]}
do
    echo "$m";
    ./resources/candidates-compare resources/workloads/${TDS}_sorted.bracket resources/results/$TDS/$TAU/$m-candidates.csv \
          topdiff $TAU > resources/results/$TDS/$TAU/$m-topdiff-verification-times-ns.csv
    ./resources/candidates-compare resources/workloads/${TDS}_sorted.bracket resources/results/$TDS/$TAU/$m-candidates.csv \
      apted $TAU > resources/results/$TDS/$TAU/$m-apted-verification-times-ns.csv
done;

# bash run_and_validate.sh $DS structural $T | grep -e ';' -e '%' > results/$DS-precision.txt