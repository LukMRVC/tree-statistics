DS=${1}
SAMPLE_SILE=${2}
BASE=resources/workloads
THRESHOLDS=${3}

cat ${BASE}/${DS}_sorted.bracket | python3 -c "import random, sys, math; lines = [l.strip() for l in sys.stdin]; print(*(f'{math.ceil(random.uniform(${THRESHOLDS}))};{s}' for s in random.sample(lines, ${SAMPLE_SILE})), sep='\n', end='')" > ${BASE}/${DS}_query_sample.csv