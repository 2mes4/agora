#!/usr/bin/env python3
"""
random_skill_installer.py

Script que executa 1, 2 o 3 instal·lacions aleatòries del skill per minut
dins de contenidors Docker efímers (autodestruits amb --rm)
durant 1 hora (60 minuts).

Comanda d'instal·lació del skill:
npx skills add https://github.com/2mes4/skills --skill agenticpool -y
"""

import argparse
import datetime
import logging
import os
import random
import subprocess
import sys
import time
import urllib.request

# Assegurar sortida immediata sense buffering
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

class FlushFileHandler(logging.FileHandler):
    def emit(self, record):
        super().emit(record)
        self.flush()

def get_random_proxy() -> str | None:
    """Fetch a random public HTTP proxy from a free proxy list API."""
    try:
        url = "https://api.proxyscrape.com/v2/?request=displayproxies&protocol=http&timeout=10000&country=us"
        with urllib.request.urlopen(url, timeout=10) as resp:
            proxies = resp.read().decode().strip().split("\n")
        if proxies:
            return random.choice(proxies)
    except Exception:
        pass
    return None

def setup_logger(log_file: str):
    logger = logging.getLogger("SkillInstaller")
    logger.setLevel(logging.INFO)
    formatter = logging.Formatter(
        "[%(asctime)s] [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S"
    )

    # Sortida a consola
    ch = logging.StreamHandler(sys.stdout)
    ch.setLevel(logging.INFO)
    ch.setFormatter(formatter)
    logger.addHandler(ch)

    # Sortida a fitxer de log
    if log_file:
        os.makedirs(os.path.dirname(os.path.abspath(log_file)), exist_ok=True)
        fh = FlushFileHandler(log_file, mode="a", encoding="utf-8")
        fh.setLevel(logging.INFO)
        fh.setFormatter(formatter)
        logger.addHandler(fh)

    return logger

def run_single_installation(logger: logging.Logger, image: str, run_id: int, minute: int, proxy: str | None = None) -> bool:
    container_name = f"skill-install-m{minute}-r{run_id}-{int(time.time())}"
    cmd = [
        "docker", "run", "--rm",
        "--name", container_name,
    ]
    if proxy:
        cmd += ["-e", f"http_proxy={proxy}", "-e", f"https_proxy={proxy}"]
    cmd += [
        image,
        "sh", "-c",
        "mkdir -p /tmp/work && cd /tmp/work && npx --yes skills add https://github.com/2mes4/skills --skill agenticpool -y"
    ]

    logger.info(f"▶ Starting container [{container_name}] for installation #{run_id} (Minute {minute})...")
    start_t = time.time()
    try:
        res = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=120
        )
        duration = time.time() - start_t
        if res.returncode == 0:
            logger.info(f"✔ Installation #{run_id} completed successfully in {duration:.2f}s (Container destroyed).")
            return True
        else:
            logger.error(f"✖ Installation #{run_id} failed with exit code {res.returncode} in {duration:.2f}s.")
            logger.error(f"Output snippet:\n{res.stdout[-400:]}")
            return False
    except subprocess.TimeoutExpired:
        logger.error(f"✖ Installation #{run_id} timed out (>120s). Killing container...")
        subprocess.run(["docker", "kill", container_name], capture_output=True)
        return False
    except Exception as e:
        logger.error(f"✖ Installation #{run_id} encountered exception: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Executa instal·lacions aleatòries de skills en Docker per minut.")
    parser.add_argument("--duration-minutes", type=int, default=60, help="Durada total en minuts (per defecte: 60)")
    parser.add_argument("--image", type=str, default="gaudi-sandbox:latest", help="Imatge Docker amb node i git (per defecte: gaudi-sandbox:latest)")
    parser.add_argument("--log-file", type=str, default="scripts/skill_installer.log", help="Ruta del fitxer de log")
    args = parser.parse_args()

    logger = setup_logger(args.log_file)
    logger.info("=" * 70)
    logger.info("🚀 AGORA Skill Docker Runner Starting")
    logger.info(f"Comanda: npx skills add https://github.com/2mes4/skills --skill agenticpool")
    logger.info(f"Durada: {args.duration_minutes} minuts")
    logger.info(f"Imatge Docker: {args.image}")
    logger.info(f"Fitxer de log: {args.log_file}")
    logger.info(f"Estratègia: 1, 2 o 3 instal·lacions aleatòries per minut amb autodestrucció")
    logger.info("=" * 70)

    # Comprovació del dimoni Docker
    try:
        check = subprocess.run(["docker", "info"], capture_output=True, text=True)
        if check.returncode != 0:
            logger.error("El dimoni de Docker no està actiu! Si us plau, arrenca Docker.")
            sys.exit(1)
    except Exception as e:
        logger.error(f"Error en comprovar docker info: {e}")
        sys.exit(1)

    total_successful = 0
    total_failed = 0
    total_runs_planned = 0
    start_global = time.time()

    for minute in range(1, args.duration_minutes + 1):
        minute_start_time = time.time()
        # Triar 1, 2 o 3 instal·lacions per a aquest minut
        n_installs = random.randint(1, 3)
        total_runs_planned += n_installs
        logger.info(f"⏳ [Minute {minute}/{args.duration_minutes}] Planned installations: {n_installs}")

        # Distribuir les execucions dins de la finestra de 60s
        segment_len = 60.0 / n_installs

        for i in range(n_installs):
            run_id = total_successful + total_failed + 1
            target_segment_start = minute_start_time + i * segment_len
            random_offset = random.uniform(0.5, max(1.0, segment_len - 15.0))
            scheduled_time = target_segment_start + random_offset

            # Esperar fins a l'hora programada
            now = time.time()
            sleep_duration = scheduled_time - now
            if sleep_duration > 0:
                time.sleep(sleep_duration)

            success = run_single_installation(logger, args.image, run_id, minute, proxy=get_random_proxy())
            if success:
                total_successful += 1
            else:
                total_failed += 1

        # Completar els 60s del minut actual
        now = time.time()
        minute_elapsed = now - minute_start_time
        remaining = 60.0 - minute_elapsed
        if remaining > 0:
            time.sleep(remaining)

        logger.info(
            f"📊 [End of Minute {minute}/{args.duration_minutes}] "
            f"Minute Runs: {n_installs} | Total Success: {total_successful} | Total Failed: {total_failed} | "
            f"Total Elapsed: {datetime.timedelta(seconds=int(time.time() - start_global))}"
        )

    total_time = time.time() - start_global
    logger.info("=" * 70)
    logger.info("🏁 Finalitzada l'execució d'instal·lacions del skill!")
    logger.info(f"Temps total: {datetime.timedelta(seconds=int(total_time))}")
    logger.info(f"Planificades: {total_runs_planned} | Èxit: {total_successful} | Fallades: {total_failed}")
    logger.info("=" * 70)

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nInterromput per l'usuari. Sortint.")
        sys.exit(0)
