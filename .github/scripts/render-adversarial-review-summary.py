#!/usr/bin/env python3
"""Render a GitHub Actions summary from agent adversarial review output."""

from __future__ import annotations

import argparse
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
        r"JSON_RESULT_START\s*(?:```(?:json)?\s*)?(.*?)(?:```\s*)?JSON_RESULT_END",
        response,
        flags=re.IGNORECASE | re.DOTALL,
    )
    candidates: list[str] = []
    if marker_match:
        candidates.append(marker_match.group(1).strip())

    candidates.extend(match.group(1).strip() for match in re.finditer(r"```json\s*(.*?)```", response, re.DOTALL | re.IGNORECASE))

    for candidate in candidates:
        try:
            parsed = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            return parsed, None

    return None, "Could not find a valid JSON review block between JSON_RESULT_START and JSON_RESULT_END."


def normalize_tests(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


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

        lines.extend([
            f"- **Recommendation:** `{recommendation}`",
            f"- **Why:** {why}",
        ])
        if final_message:
            lines.append(f"- **Final message:** {final_message}")
        lines.extend(["", "### Structured probes", ""])

        if not tests:
            lines.extend(["No structured probes were parsed from the agent response.", ""])
        else:
            for index, test in enumerate(tests, start=1):
                title = str(test.get("title", f"Probe {index}"))
                result = str(test.get("result", "UNKNOWN"))
                lines.extend([
                    f"#### {index}. {title} — `{result}`",
                    "",
                    f"- **Hypothesis:** {test.get('hypothesis', '')}",
                    f"- **Impact:** {test.get('impact', '')}",
                    f"- **Command:** `{test.get('command', '')}`",
                    f"- **Output:** {test.get('output', '')}",
                    f"- **Coverage recommendation:** {test.get('unitTestRecommendation', '')}",
                    "",
                ])

    lines.extend([
        "### Full agent response",
        "",
        "<details>",
        "<summary>Expand raw response</summary>",
        "",
        "````text",
        response[-12000:] if response else "(no agent response captured)",
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
