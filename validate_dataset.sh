#!/bin/bash

TDS=$1
TAU=$2

methods=("structural" "structural-variant" "sed" "lblint")


for m in ${methods[@]}
do
    echo "$m";
    mkdir -p results/$TDS/$TAU
    bash run_and_validate.sh $TDS $m $TAU | grep -e ';' -e '%' > resources/results/$TDS/$TAU/$TDS-$m-precision.txt
done;

# bash run_and_validate.sh $DS structural $T | grep -e ';' -e '%' > results/$DS-precision.txt