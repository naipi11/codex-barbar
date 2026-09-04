#!/usr/bin/python3
"""Deterministic JSONL Codex App Server contract fixture for Linux tests."""

import argparse
import json
import sys
import time


MODES = {
    "normal", "interleaved", "out-of-order", "unknown-notification",
    "duplicate-id", "invalid-json", "truncated", "oversized",
    "initialize-timeout", "rpc-timeout", "crash", "refuse-exit",
    "login-failed", "login-cancelled",
}


def write_raw(text):
    sys.stdout.write(text)
    sys.stdout.flush()


def write_frame(value):
    write_raw(json.dumps(value, separators=(",", ":")) + "\n")


def response(request_id, result):
    write_frame({"id": request_id, "result": result})


def error_response(request_id, code):
    write_frame({"id": request_id, "error": {"code": code, "message": "fixture error"}})


def account_result():
    return {"account": {"type": "chatgpt", "email": "fixture@example.invalid", "planType": "plus"}}


def rate_limits_result():
    return {
        "rateLimitsByLimitId": {
            "codex": {
                "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": 4102444800},
                "secondary": {"usedPercent": 10, "windowDurationMins": 10080, "resetsAt": 4102444800},
            }
        }
    }


def emit_unknown(mode):
    if mode == "interleaved":
        write_frame({"method": "account/updated", "params": {"marker": "known"}})
    write_frame({"method": "fixture/unknown", "params": {"marker": "ignored"}})


def held_result(request):
    if request.get("method") == "account/read":
        return account_result()
    return rate_limits_result()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", required=True, choices=sorted(MODES))
    parser.add_argument("remaining", nargs="*")
    mode = parser.parse_args().mode
    held_request = None
    initialized = False

    for line in sys.stdin:
        if not line.strip():
            continue
        if mode == "invalid-json":
            write_raw('{"id":1,"result":\n')
            continue
        if mode == "truncated":
            write_raw('{"id":1,"result":')
            break
        if mode == "oversized":
            write_raw("x" * 1048577 + "\n")
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            write_raw('{"id":1,"error":{"code":-32700,"message":"invalid request"}}')
            continue

        request_id = request.get("id")
        method = request.get("method", "")
        if method == "initialized" and request_id is None:
            initialized = True
            continue
        if method == "initialize":
            if mode == "initialize-timeout":
                time.sleep(60)
                continue
            if request_id != 1 or request.get("params", {}).get("experimentalApi") is not False:
                error_response(request_id, -32602)
            else:
                response(request_id, {"serverInfo": {"name": "fake-codex", "version": "0.0.0-test"}})
            continue

        if method in {"account/read", "account/rateLimits/read"}:
            if not initialized:
                error_response(request_id, -32001)
                continue
            if mode == "crash":
                raise SystemExit(17)
            if mode == "rpc-timeout":
                time.sleep(60)
                continue
            current_result = account_result() if method == "account/read" else rate_limits_result()
            if mode == "out-of-order":
                if held_request is None:
                    held_request = request
                else:
                    response(request_id, current_result)
                    response(held_request.get("id"), held_result(held_request))
                    held_request = None
                continue
            if mode in {"unknown-notification", "interleaved"}:
                emit_unknown(mode)
            response(request_id, current_result)
            if mode == "duplicate-id":
                response(request_id, current_result)
            continue

        if method == "account/login/start":
            login_type = request.get("params", {}).get("type")
            if login_type == "chatgpt":
                response(request_id, {"loginId": "login-browser", "authorizationUrl": "https://auth.openai.com/authorize?client=codex-barbar"})
                if mode == "login-failed":
                    write_frame({"method": "account/login/failed", "params": {"loginId": "login-browser", "error": "fixture secret text"}})
                elif mode == "login-cancelled":
                    write_frame({"method": "account/login/cancelled", "params": {"loginId": "login-browser"}})
                else:
                    write_frame({"method": "account/login/completed", "params": {"loginId": "login-browser"}})
            elif login_type == "chatgptDeviceCode":
                response(request_id, {"loginId": "login-device", "verificationUrl": "https://auth.openai.com/codex/device", "userCode": "ABCD-EFGH"})
            else:
                error_response(request_id, -32602)
            continue

        if method == "account/login/cancel":
            login_id = request.get("params", {}).get("loginId", "")
            response(request_id, {"cancelled": True})
            write_frame({"method": "account/login/cancelled", "params": {"loginId": login_id}})
            continue

        error_response(request_id, -32601)

    if mode == "refuse-exit":
        while True:
            time.sleep(0.1)


if __name__ == "__main__":
    main()
