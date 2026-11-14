#!/usr/bin/env python3
"""Load the local .sqsrc credential file for simple_SQS.fifo."""

import os
import warnings
from pathlib import Path

warnings.filterwarnings("ignore", message="Boto3 will no longer support Python 3.9")

import boto3

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = (
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_DEFAULT_REGION",
    "SQS_QUEUE_URL",
)


def load_sqsrc(path=None):
    path = Path(path or os.environ.get("SQSRC") or ROOT / ".sqsrc")
    if not path.is_file():
        raise SystemExit(f"missing {path}")
    cfg = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        cfg[key.strip()] = value.strip().strip('"').strip("'")
    missing = [key for key in REQUIRED if not cfg.get(key)]
    if missing:
        raise SystemExit(f"{path} missing {', '.join(missing)}")
    return cfg


def client(cfg):
    return boto3.client(
        "sqs",
        region_name=cfg["AWS_DEFAULT_REGION"],
        aws_access_key_id=cfg["AWS_ACCESS_KEY_ID"],
        aws_secret_access_key=cfg["AWS_SECRET_ACCESS_KEY"],
    )
