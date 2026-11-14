#!/usr/bin/env python3
"""Long-poll simple_SQS.fifo and delete each message after printing it.

Uses the key in .sqsrc. Keeps one ReceiveMessage call open; SQS returns as
soon as a message is available. Delete is the acknowledgement.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from sqs_common import client, load_sqsrc


def main():
    parser = argparse.ArgumentParser(description="Long-poll simple_SQS.fifo")
    parser.add_argument("--once", action="store_true", help="return after one batch or one empty poll")
    parser.add_argument("--wait", type=int, default=20, help="long-poll wait in seconds, max 20")
    parser.add_argument("--max", type=int, default=1, help="messages to take per call, max 10")
    args = parser.parse_args()
    if not 0 <= args.wait <= 20:
        raise SystemExit("--wait must be from 0 to 20")
    if not 1 <= args.max <= 10:
        raise SystemExit("--max must be from 1 to 10")

    cfg = load_sqsrc()
    sqs = client(cfg)
    url = cfg["SQS_QUEUE_URL"]
    while True:
        response = sqs.receive_message(
            QueueUrl=url,
            MaxNumberOfMessages=args.max,
            WaitTimeSeconds=args.wait,
            AttributeNames=["All"],
        )
        messages = response.get("Messages", [])
        if not messages:
            print("empty")
            if args.once:
                return
            continue
        for message in messages:
            attrs = message.get("Attributes", {})
            print(
                f"message_id={message['MessageId']} "
                f"sequence={attrs.get('SequenceNumber', '')} "
                f"group_id={attrs.get('MessageGroupId', '')} "
                f"body={message['Body']}"
            )
            sqs.delete_message(QueueUrl=url, ReceiptHandle=message["ReceiptHandle"])
        if args.once:
            return


if __name__ == "__main__":
    main()
