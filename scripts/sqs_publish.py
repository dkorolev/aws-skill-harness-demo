#!/usr/bin/env python3
"""Publish one message to simple_SQS.fifo.

Uses the key in .sqsrc. --id and --body are required. A repeated id within
five minutes is accepted and does not create a second message. Every message
uses one group id so the queue keeps a single total order.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from sqs_common import client, load_sqsrc


def main():
    parser = argparse.ArgumentParser(description="Publish one message to simple_SQS.fifo")
    parser.add_argument("--id", required=True, help="idempotency token; required on every send")
    parser.add_argument("--body", required=True, help="message body; required on every send")
    parser.add_argument("--group-id", default="main", help="one value keeps a single total order")
    args = parser.parse_args()
    message_id = args.id.strip()
    body = args.body.strip()
    if not message_id:
        raise SystemExit("--id is required")
    if not body:
        raise SystemExit("--body is required")

    cfg = load_sqsrc()
    response = client(cfg).send_message(
        QueueUrl=cfg["SQS_QUEUE_URL"],
        MessageBody=body,
        MessageGroupId=args.group_id,
        MessageDeduplicationId=message_id,
    )
    print(
        f"message_id={response['MessageId']} "
        f"sequence={response['SequenceNumber']} "
        f"id={message_id} "
        f"group_id={args.group_id}"
    )


if __name__ == "__main__":
    main()
