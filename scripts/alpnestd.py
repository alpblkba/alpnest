#!/usr/bin/env python3
"""Tiny alpnest daemon loop.

For now this daemon only keeps the generated mail view fresh from the local
JSON store. Later it will call the Apple Mail collector, calendar collector,
today generator, and notification worker.
"""

from __future__ import annotations

import argparse
import logging
import signal
import sys
import time
from pathlib import Path

import bootstrap_data
import generate_mail_view
from paths import LOG_DIR

DEFAULT_INTERVAL_SECONDS = 60
LOG_FILE = LOG_DIR / "alpnestd.log"

should_stop = False


def configure_logging(verbose: bool) -> None:
    LOG_DIR.mkdir(parents=True, exist_ok=True)

    level = logging.DEBUG if verbose else logging.INFO
    handlers: list[logging.Handler] = [
        logging.FileHandler(LOG_FILE, encoding="utf-8"),
        logging.StreamHandler(sys.stdout),
    ]

    logging.basicConfig(
        level=level,
        format="%(asctime)s %(levelname)s %(message)s",
        handlers=handlers,
    )


def handle_signal(signum: int, _frame: object) -> None:
    global should_stop
    should_stop = True
    logging.info("received signal %s; stopping alpnestd", signum)


def run_once() -> None:
    """Run one daemon cycle."""
    logging.debug("starting daemon cycle")

    bootstrap_data.main()
    generate_mail_view.main()

    logging.debug("finished daemon cycle")


def sleep_interruptibly(seconds: int) -> None:
    """Sleep in small chunks so Ctrl-C / signals stop quickly."""
    remaining = max(seconds, 0)

    while remaining > 0 and not should_stop:
        time.sleep(min(1, remaining))
        remaining -= 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the alpnest local daemon loop.")
    parser.add_argument(
        "--interval",
        type=int,
        default=DEFAULT_INTERVAL_SECONDS,
        help=f"seconds between daemon cycles; default: {DEFAULT_INTERVAL_SECONDS}",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="run one cycle and exit",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="enable debug logging",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    configure_logging(args.verbose)

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    logging.info("starting alpnestd")
    logging.info("log file: %s", LOG_FILE)
    logging.info("interval: %s seconds", args.interval)

    if args.once:
        run_once()
        logging.info("finished one-shot daemon cycle")
        return 0

    while not should_stop:
        try:
            run_once()
        except Exception:
            logging.exception("daemon cycle failed")

        sleep_interruptibly(args.interval)

    logging.info("alpnestd stopped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
