#!/usr/bin/env python3
"""tiny alpnest daemon loop."""

from __future__ import annotations

import argparse
import configparser
import logging
import signal
import sys
import time
from pathlib import Path

import bootstrap_data
import generate_mail_decomposition
import generate_mail_view
import sync_mail_apple
from paths import LOG_DIR

CONFIG_FILE = Path(__file__).with_name("alpnestd.cfg")

DEFAULT_INTERVAL_SECONDS = 300
DEFAULT_MAIL_LIMIT = 3
DEFAULT_INCLUDE_MAIL_BODY = False
LOG_FILE = LOG_DIR / "alpnestd.log"

should_stop = False


def read_config() -> configparser.ConfigParser:
    config = configparser.ConfigParser()
    config["daemon"] = {
        "interval_seconds": str(DEFAULT_INTERVAL_SECONDS),
    }
    config["mail"] = {
        "enabled": "true",
        "limit": str(DEFAULT_MAIL_LIMIT),
        "include_body": str(DEFAULT_INCLUDE_MAIL_BODY).lower(),
    }

    if CONFIG_FILE.exists():
        config.read(CONFIG_FILE)

    return config


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


def run_mail_sync(limit: int, include_body: bool) -> None:
    original_argv = sys.argv[:]

    try:
        sys.argv = [
            "sync_mail_apple.py",
            "--default-targets",
            "--limit",
            str(limit),
        ]

        if include_body:
            sys.argv.append("--include-body")

        sync_mail_apple.main()
    finally:
        sys.argv = original_argv


def run_once(sync_mail: bool, mail_limit: int, include_mail_body: bool) -> None:
    logging.debug("starting daemon cycle")

    bootstrap_data.main()

    if sync_mail:
        run_mail_sync(mail_limit, include_mail_body)

    generate_mail_view.main()
    generate_mail_decomposition.main()

    logging.debug("finished daemon cycle")


def sleep_interruptibly(seconds: int) -> None:
    remaining = max(seconds, 0)

    while remaining > 0 and not should_stop:
        time.sleep(min(1, remaining))
        remaining -= 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the alpnest local daemon loop.")
    parser.add_argument(
        "--interval",
        type=int,
        help="override configured daemon loop interval in seconds",
    )
    parser.add_argument(
        "--mail-limit",
        type=int,
        help="override configured recent mail messages per account",
    )
    parser.add_argument(
        "--no-mail-sync",
        action="store_true",
        help="skip Apple Mail sync and only regenerate views",
    )
    parser.add_argument(
        "--include-mail-body",
        action="store_true",
        help="force full body sync for the small mail batch",
    )
    parser.add_argument(
        "--metadata-only-mail",
        action="store_true",
        help="force metadata-only mail sync",
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
    config = read_config()
    configure_logging(args.verbose)

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    interval = args.interval if args.interval is not None else config.getint("daemon", "interval_seconds")
    sync_mail = config.getboolean("mail", "enabled") and not args.no_mail_sync
    mail_limit = args.mail_limit if args.mail_limit is not None else config.getint("mail", "limit")

    include_mail_body = config.getboolean("mail", "include_body")
    if args.include_mail_body:
        include_mail_body = True
    if args.metadata_only_mail:
        include_mail_body = False

    logging.info("starting alpnestd")
    logging.info("config file: %s", CONFIG_FILE)
    logging.info("log file: %s", LOG_FILE)
    logging.info("interval: %s seconds", interval)
    logging.info("mail sync: %s", sync_mail)
    logging.info("mail limit: %s", mail_limit)
    logging.info("mail body sync: %s", include_mail_body)

    if args.once:
        run_once(sync_mail=sync_mail, mail_limit=mail_limit, include_mail_body=include_mail_body)
        logging.info("finished one-shot daemon cycle")
        return 0

    while not should_stop:
        try:
            run_once(sync_mail=sync_mail, mail_limit=mail_limit, include_mail_body=include_mail_body)
        except Exception:
            logging.exception("daemon cycle failed")

        sleep_interruptibly(interval)

    logging.info("alpnestd stopped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
