#!/usr/bin/env python3
import os
import sys

SENTIMENT_DATA = 9645.0

def print_first_diff(dataset, diff):
    first_diff = diff[0]
    query_id, data_id = first_diff.split(",")
    query = ""
    data = ""
    with open(f"resources/workloads/{dataset}_query_sample.csv") as f:
        query_id = int(query_id)
        for idx, query_string in enumerate(f):
            if idx == query_id:
                query = query_string
                break

    with open(f"resources/workloads/{dataset}_sorted.bracket") as f:
        data_id = int(data_id)
        for idx, data_string in enumerate(f):
            if idx == data_id:
                data = data_string
                break

    print(diff)
    print(query)
    print(data)


def main(dataset: str):
    with open(f"resources/results/{dataset}/Lblint_candidates.csv") as f:
        lblint = [l.strip() for l in f]

    with open(f"resources/results/{dataset}/Lblint_index_candidates.csv") as f:
        lblint_index = [l.strip() for l in f]

    diff = list(set(lblint) - set(lblint_index))
    if len(diff) > 0:
        print_first_diff(dataset, diff)
    else:
        print('Contents are identical')

    
    query_results = dict()
    for res in lblint_index:
        q, _ = res.split(',')
        if q not in query_results:
            query_results[q] = 0
        query_results[q] += 1

    sels = []
    modify_queries = []

    for q, cnt in query_results.items():
        selectivity = (float(cnt) / SENTIMENT_DATA) * 100
        sels.append(selectivity)
        print(f'Query {q}: {selectivity:.2f}%')
        # if selectivity < 0.1:
        #     modify_queries.append(int(q))


    avg_sel = sum(sels) / len(sels)
    print(f'Avg. selectivity is: {avg_sel:.2f}%')
    # print(modify_queries)
    # with open(f"resources/workloads/{dataset}_query_sample.csv", 'r+') as f:
    #     queries = []
    #     for idx, query_string in enumerate(f):
    #         queries.append(query_string.split(';'))

    #     for modify in modify_queries:
    #         new_t = int(queries[modify][0]) + 5
    #         queries[modify][0] = str( new_t if new_t < 20 else 20 )

    #     content = '\n'.join([';'.join(q) for q in queries])
    #     f.seek(0)
    #     f.write(content)

    

if __name__ == "__main__":
    dataset = sys.argv[1]
    main(dataset)
