from collections import defaultdict

with open("./resources/results/treefam/query_verified.csv") as f:
    content = f.read()

qm = defaultdict(int)
km = dict()

for i, line in enumerate(content.split("\n")):
    try:
        [t1, rest] = line.split(",")
    except ValueError:
        print("Err on line", i)
        # raise
        continue
    [t2, *rest] = rest.split(" ")
    k = rest[-1]
    print(t1, t2, k)
    qm[t1] += 1
    km[t1] = k


final_list = [int(k) for k, v in qm.items() if 50 <= v < 150]


print(final_list)
print(len(final_list))

fqm = dict()

try:
    with open("./resources/results/treefam/final_queries.csv") as f:
        for line in f:
            t, q = line.strip().split("\t")
            fqm[q] = int(t)
except FileNotFoundError:
    pass


with open("./resources/workloads/treefam_query_sample.csv") as f:
    lines = [l.strip() for l in f]
    for qid in final_list:
        k, q = lines[qid].split(";")
        fqm[q] = min(int(k), fqm.get(q, 9999999))


with open("./resources/results/treefam/final_queries.csv", "w") as f:
    for q, t in fqm.items():
        f.write(f"{t}\t{q}\n")
