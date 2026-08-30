#!/usr/bin/env python3
"""
counter_stress_test.py

Stress test for your own skill-counter backend. Validates that the counter
increments correctly per unique client IP by sending N requests from N
simulated distinct client addresses.

This targets YOUR endpoint directly. It does not touch npm, GitHub, or any
third-party service, and it does not route traffic through proxies.

Prerequisite: your backend must read the client IP from a forwarded header
(e.g. X-Forwarded-For / X-Real-IP), as is typical behind a reverse proxy or
in a staging environment. If your backend reads the raw TCP source IP, this
tool cannot help (see README notes at the end of this file).

Usage examples:
    python3 scripts/counter_stress_test.py \\
        --endpoint https://agenticpool.net/api/v1/installs \\
        --requests 300 --unique-ips 150 --concurrency 20

    python3 scripts/counter_stress_test.py \\
        --endpoint http://localhost:8080/api/v1/installs \\
        --method POST --requests 60 --unique-ips 60 --concurrency 10
"""

import argparse
import itertools
import random
import statistics
import sys
import threading
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(line_buffering=True)

_results_lock = threading.Lock()
_results = {"ok": 0, "failed": 0, "latencies": []}


def generate_ips(count: int) -> list[str]:
    """Generate `count` distinct, plausible-looking private test IPs."""
    ips = set()
    while len(ips) < count:
        ips.add(".".join(str(random.randint(2, 254)) for _ in range(4)))
    return sorted(ips)


def send_request(
    endpoint: str,
    method: str,
    client_ip: str,
    ip_header: str,
    timeout: float,
) -> tuple[bool, float]:
    req = urllib.request.Request(endpoint, method=method)
    req.add_header(ip_header, client_ip)
    req.add_header("X-Real-IP", client_ip)
    req.add_header("User-Agent", "agora-counter-stress-test/1.0")
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            status = resp.status
        ok = 200 <= status < 300
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as e:
        return False, time.perf_counter() - start
    return ok, time.perf_counter() - start


def worker(
    endpoint: str,
    method: str,
    ips: list[str],
    ip_header: str,
    timeout: float,
    total: int,
    request_counter: itertools.count,
):
    for _ in range(total):
        client_ip = ips[next(request_counter) % len(ips)]
        ok, latency = send_request(endpoint, method, client_ip, ip_header, timeout)
        with _results_lock:
            _results["ok" if ok else "failed"] += 1
            _results["latencies"].append(latency)


def main():
    parser = argparse.ArgumentParser(description="Stress test del comptador de skills propi (per IP única).")
    parser.add_argument("--endpoint", required=True, help="URL del teu endpoint de comptatge, ex. https://agenticpool.net/api/v1/installs")
    parser.add_argument("--method", default="GET", choices=["GET", "POST", "PUT"])
    parser.add_argument("--requests", type=int, default=100, help="Nombre total de peticions")
    parser.add_argument("--unique-ips", type=int, default=50, help="Nombre d'IPs de client diferents a simular (ha de ser >= 1)")
    parser.add_argument("--concurrency", type=int, default=10, help="Peticions simultànies")
    parser.add_argument("--ip-header", default="X-Forwarded-For", help="Capçalera on el backend llegeix la IP del client")
    parser.add_argument("--timeout", type=float, default=15.0, help="Timeout per petició (segons)")
    args = parser.parse_args()

    if args.requests < 1 or args.unique_ips < 1:
        sys.exit("--requests i --unique-ips han de ser >= 1")
    if args.unique_ips > args.requests:
        print(f"Nota: --unique-ips ({args.unique_ips}) > --requests ({args.requests}); s'usaran com a màxim {args.requests} IPs.")
        args.unique_ips = args.requests

    ips = generate_ips(args.unique_ips)
    print(f"Endpoint:  {args.endpoint}")
    print(f"Method:    {args.method}")
    print(f"Requests:  {args.requests} (concurrency {args.concurrency})")
    print(f"Unique IPs: {len(ips)} via {args.ip_header}")
    print("-" * 60)

    request_counter = itertools.count()
    per_worker = args.requests // args.concurrency
    remainder = args.requests % args.concurrency
    workers = []
    for w in range(args.concurrency):
        total = per_worker + (1 if w < remainder else 0)
        if total == 0:
            continue
        t = threading.Thread(
            target=worker,
            args=(args.endpoint, args.method, ips, args.ip_header, args.timeout, total, request_counter),
            daemon=True,
        )
        workers.append(t)

    start_global = time.perf_counter()
    for t in workers:
        t.start()
    for t in workers:
        t.join()
    elapsed = time.perf_counter() - start_global

    with _results_lock:
        ok = _results["ok"]
        failed = _results["failed"]
        latencies = sorted(_results["latencies"])

    print("-" * 60)
    print(f"OK:      {ok}")
    print(f"Failed:  {failed}")
    print(f"Total:   {ok + failed} en {elapsed:.1f}s ({ (ok + failed) / elapsed:.1f} req/s)")
    if latencies:
        print(f"Latency p50: {statistics.median(latencies) * 1000:.0f}ms | p95: {latencies[int(len(latencies) * 0.95) - 1] * 1000:.0f}ms")
    print("=" * 60)
    print("Validació: compara el comptador al teu backend amb el valor esperat:")
    print(f"  - Si compta per IP única: hauria d'incrementar {len(ips)} vegades (una per IP).")
    print(f"  - Si no diferencia IPs: incrementarà {args.requests} vegades (bug de dedup per IP).")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
