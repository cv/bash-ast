#!/usr/bin/env python3
"""Render a GitHub Actions summary from agent adversarial review output."""

from __future__ import annotations

import argparse
import html
import json
import re
from pathlib import Path
from typing import Any


def read_text(path: Path | None) -> str:
    if path is None or not path.exists():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def extract_json_blob(response: str) -> tuple[dict[str, Any] | None, str | None]:
    marker_match = re.search(
        r"^\s*JSON_RESULT_START\s*$\s*(.*?)^\s*JSON_RESULT_END\s*$",
        response,
        flags=re.IGNORECASE | re.DOTALL | re.MULTILINE,
    )
    if not marker_match:
        return None, "Could not find JSON_RESULT_START / JSON_RESULT_END markers."

    candidate = marker_match.group(1).strip()
    fence_match = re.fullmatch(r"```(?:json)?\s*(.*?)\s*```", candidate, flags=re.IGNORECASE | re.DOTALL)
    if fence_match:
        candidate = fence_match.group(1).strip()

    try:
        parsed = json.loads(candidate)
    except json.JSONDecodeError as error:
        return None, f"Could not parse JSON review block between JSON_RESULT_START and JSON_RESULT_END: {error}"

    if not isinstance(parsed, dict):
        return None, "Structured review JSON must be an object."
    return parsed, None


def validate_review(review: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    recommendation = review.get("recommendation")
    if recommendation not in {"PASS", "FAIL", "INVESTIGATE"}:
        errors.append("recommendation must be PASS, FAIL, or INVESTIGATE")

    for field in ("why", "finalMessage"):
        if not isinstance(review.get(field), str) or not review[field].strip():
            errors.append(f"{field} must be a non-empty string")

    tests = review.get("tests")
    if not isinstance(tests, list) or not tests:
        errors.append("tests must be a non-empty array")
        return errors

    required_test_fields = ("title", "hypothesis", "impact", "command", "output", "result", "unitTestRecommendation")
    for index, test in enumerate(tests, start=1):
        if not isinstance(test, dict):
            errors.append(f"tests[{index}] must be an object")
            continue
        for field in required_test_fields:
            if not isinstance(test.get(field), str) or not test[field].strip():
                errors.append(f"tests[{index}].{field} must be a non-empty string")
        if test.get("result") not in {"PASS", "FAIL"}:
            errors.append(f"tests[{index}].result must be PASS or FAIL")

    return errors


def normalize_tests(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def markdown_text(value: Any) -> str:
    return html.escape(str(value), quote=False)


def fenced_text(value: Any) -> str:
    return html.escape(str(value), quote=False).replace("```", "`\\`\\`")


def append_log_tail(lines: list[str], title: str, text: str, max_lines: int = 60) -> None:
    if not text.strip():
        return
    tail = "\n".join(text.rstrip().splitlines()[-max_lines:])
    lines.extend([
        f"### {title}",
        "",
        "```text",
        tail,
        "```",
        "",
    ])


def render_summary(args: argparse.Namespace) -> str:
    response = read_text(args.response)
    build_log = read_text(args.build_log)
    baseline_log = read_text(args.baseline_log)
    build_status = read_text(args.build_status).strip() if args.build_status and args.build_status.exists() else "unknown"
    baseline_status = read_text(args.baseline_status).strip() if args.baseline_status and args.baseline_status.exists() else "unknown"
    review, warning = extract_json_blob(response)

    lines: list[str] = [
        "## Adversarial review",
        "",
        f"- **Build exit code:** `{build_status or 'unknown'}`",
        f"- **Baseline test exit code:** `{baseline_status or 'unknown'}`",
    ]

    if review is None:
        lines.extend([
            "- **Recommendation:** `UNKNOWN`",
            "",
            f"> ⚠️ {warning}",
            "",
        ])
    else:
        recommendation = str(review.get("recommendation", "UNKNOWN"))
        why = str(review.get("why", "No rationale supplied."))
        final_message = str(review.get("finalMessage", ""))
        tests = normalize_tests(review.get("tests"))
        validation_errors = validate_review(review)

        lines.extend([
            f"- **Recommendation:** `{markdown_text(recommendation)}`",
            f"- **Why:** {markdown_text(why)}",
        ])
        if final_message:
            lines.append(f"- **Final message:** {markdown_text(final_message)}")
        if validation_errors:
            lines.extend(["", "### Structured review validation errors", ""])
            lines.extend(f"- {error}" for error in validation_errors)
        lines.extend(["", "### Structured probes", ""])

        if not tests:
            lines.extend(["No structured probes were parsed from the agent response.", ""])
        else:
            for index, test in enumerate(tests, start=1):
                title = str(test.get("title", f"Probe {index}"))
                result = str(test.get("result", "UNKNOWN"))
                lines.extend([
                    f"#### {index}. {markdown_text(title)} — `{markdown_text(result)}`",
                    "",
                    f"- **Hypothesis:** {markdown_text(test.get('hypothesis', ''))}",
                    f"- **Impact:** {markdown_text(test.get('impact', ''))}",
                    "- **Command:**",
                    "",
                    "```text",
                    fenced_text(test.get('command', '')),
                    "```",
                    "",
                    f"- **Output:** {markdown_text(test.get('output', ''))}",
                    f"- **Coverage recommendation:** {markdown_text(test.get('unitTestRecommendation', ''))}",
                    "",
                ])

    lines.extend([
        "### Full agent response",
        "",
        "<details>",
        "<summary>Expand raw response</summary>",
        "",
        "````text",
        fenced_text(response[-12000:]) if response else "(no agent response captured)",
        "````",
        "",
        "</details>",
        "",
    ])

    append_log_tail(lines, "Build log tail", build_log)
    append_log_tail(lines, "Baseline test log tail", baseline_log)

    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--response", type=Path, required=True)
    parser.add_argument("--build-log", type=Path)
    parser.add_argument("--baseline-log", type=Path)
    parser.add_argument("--build-status", type=Path)
    parser.add_argument("--baseline-status", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render_summary(args), encoding="utf-8")


if __name__ == "__main__":
    main()
