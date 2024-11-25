#!/usr/bin/env python3

import asyncio
import subprocess
import os
import csv
from datetime import timedelta
import sys


async def run_process(dataset: str, qgram_size):
    # -d  lower-bound -q resources/workloads/${1}_query_sample.csv --output ${RESULTS} ${QSIZE} ${MT}
    program = "./target/release/tree-statistics"
    ds_path = f"resources/workloads/{dataset}_sorted.bracket"
    query_path = f"resources/workloads/{dataset}_query_sample.csv"
    output_path = f"resources/results/{dataset}/"
    # --qgram - size

    os.makedirs(output_path, exist_ok=True)

    cmd = [
        program,
        "--quiet",
        "-d",
        ds_path,
        "lower-bound",
        "-q",
        query_path,
        "--output",
        output_path,
        "--qgram-size",
        str(qgram_size),
    ]

    process = await asyncio.create_subprocess_exec(
        *cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    stdout, stderr = await process.communicate()

    return [process.returncode, stdout.decode().strip(), stderr.decode().strip()]


async def main():
    datasets = [
        ("sentiment", 2),
        ("ptb", 2),
        ("dblp", 2),
        ("swissprot", 8),
        ("python", 6),
        ("treefam", 4),
    ]
    header = [
        "dataset",
        "lblindex",
        "lblint",
        "sedindex",
        "sed",
        "structuralindex",
        "structural",
    ]

    # ptb;12;74;59;179;25;184
    # ptb;372;372;245;245;322;322

    for ds, qgram_size in datasets:
        print("Executing", ds, "with qgram-size=", qgram_size)
        output_path = f"resources/results/{ds}/times.txt"

        [retcode, stdout, stderr] = await run_process(ds, qgram_size)
        print(ds, retcode)
        lines = stdout.split("\n")
        times = [ds]
        candidates = [ds]
        for t in range(1, len(lines), 3):
            [_, _time] = lines[t].split(":")
            _time = _time.replace("ms", "")
            times.append(_time)
        for c in range(2, len(lines), 3):
            [_, _candidates] = lines[c].split(":")
            _candidates = _candidates.replace("ms", "")
            candidates.append(_candidates)

        with open(output_path, "w") as f:
            writer = csv.writer(f, delimiter=";")
            writer.writerow(header)
            writer.writerow(times)
            writer.writerow(candidates)


if __name__ == "__main__":
    asyncio.run(main())
