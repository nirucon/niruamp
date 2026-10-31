#!/usr/bin/env python3
"""Sample CPU and RSS for one or more processes from /proc.

Usage: scripts/measure.py SECONDS PID [PID...]
Output is a compact table plus JSON on stdout.
"""
import json
import os
import sys
import time

HZ = os.sysconf("SC_CLK_TCK")


def read_stat(pid):
    with open(f"/proc/{pid}/stat", "r", encoding="utf-8") as f:
        parts = f.read().split()
    # Fields after the parenthesised comm: state is field 3, utime/stime 14/15.
    return int(parts[13]) + int(parts[14])


def read_rss(pid):
    try:
        with open(f"/proc/{pid}/status", "r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except OSError:
        return 0
    return 0


def name(pid):
    try:
        return open(f"/proc/{pid}/comm", encoding="utf-8").read().strip()
    except OSError:
        return "gone"


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    duration = float(sys.argv[1])
    pids = [int(x) for x in sys.argv[2:]]
    baseline = {pid: read_stat(pid) for pid in pids if os.path.exists(f"/proc/{pid}")}
    peak = {pid: read_rss(pid) for pid in pids}
    start = time.monotonic()
    while time.monotonic() - start < duration:
        for pid in pids:
            peak[pid] = max(peak.get(pid, 0), read_rss(pid))
        time.sleep(0.5)
    elapsed = time.monotonic() - start
    rows = []
    for pid in pids:
        if not os.path.exists(f"/proc/{pid}"):
            rows.append({"pid": pid, "name": name(pid), "cpu_percent": None, "rss_kib_peak": peak.get(pid, 0)})
            continue
        delta = read_stat(pid) - baseline.get(pid, read_stat(pid))
        cpu = (delta / HZ) / elapsed * 100.0
        rows.append({"pid": pid, "name": name(pid), "cpu_percent": round(cpu, 4), "rss_kib_peak": peak.get(pid, 0)})
    print(f"interval={elapsed:.1f}s")
    print(f"{'pid':>7} {'name':<12} {'cpu%':>8} {'rss MiB':>9}")
    for r in rows:
        cpu = "-" if r["cpu_percent"] is None else f"{r['cpu_percent']:.3f}"
        print(f"{r['pid']:>7} {r['name']:<12} {cpu:>8} {r['rss_kib_peak']/1024:>9.1f}")
    print(json.dumps(rows))


if __name__ == "__main__":
    main()
