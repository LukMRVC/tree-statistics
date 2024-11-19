DS=${1}
SAMPLE_SIZE=${2}
BASE=resources/workloads
THRESHOLDS=${3}

cat ${BASE}/${DS}_sorted.bracket | python3 -c "import random, sys, math; lines = [l.strip() for l in sys.stdin]; print(*(f'{math.ceil(random.uniform(${THRESHOLDS}))};{s}' for s in random.sample(lines, ${SAMPLE_SIZE})), sep='\n', end='')" > ${BASE}/${DS}_query_sample.csv