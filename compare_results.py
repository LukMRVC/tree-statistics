#!/usr/bin/env python3
import os
import sys


def main():
    with open("resources/results/dblp/Lblint_candidates.csv") as f:
        lblint = [l.strip() for l in f]

    with open("resources/results/dblp/Lblint_index_candidates.csv") as f:
        lblint_index = [l.strip() for l in f]

    diff = list(set(lblint) - set(lblint_index))
    first_diff = diff[0]
    query_id, data_id = first_diff.split(",")
    query = ""
    data = ""
    with open("resources/workloads/dblp_query_sample.csv") as f:
        query_id = int(query_id)
        for idx, query_string in enumerate(f):
            if idx == query_id:
                query = query_string
                break

    with open("resources/workloads/dblp_sorted.bracket") as f:
        data_id = int(data_id)
        for idx, data_string in enumerate(f):
            if idx == data_id:
                data = data_string
                break

    print(diff)
    print(query)
    print(data)


if __name__ == "__main__":
    main()
