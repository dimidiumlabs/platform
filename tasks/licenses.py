#!/usr/bin/env -S pipx run --backend pip
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: 0BSD
# fmt: off
#MISE description="Verify repository licensing metadata"
#MISE tools={"pipx"="1.16.7","python"="3.14.7","pipx:reuse"="6.2.0","aqua:EmbarkStudios/cargo-deny"="0.19.0"}
# fmt: on
# /// script
# requires-python = ">=3.11"
# dependencies = ["shellous==0.42.0"]
# ///

from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Sequence
from pathlib import Path

sys.dont_write_bytecode = True

from libs.common import TaskError, capture, run, task_main

TASK = "licenses"
HEADER = (
    r"^((<!--|#|//|/\*|\*)[[:space:]]*)?"
    r"(Copyright[[:space:]]+(\([cC]\)|©)|SPDX-FileCopyrightText:)"
)
LEGACY_COPYRIGHT = re.compile(r"^\s*((<!--|#|//|/\*|\*)\s*)?Copyright\s+(\([cC]\)|©)")
CANONICAL_COPYRIGHT = re.compile(
    r"SPDX-FileCopyrightText: 2026 Nikolay Govorov(?:\s*(?:\*/|-->))?$"
)


async def check_copyright_headers() -> None:
    result = await capture.result(
        "git",
        "grep",
        "-n",
        "-I",
        "-E",
        HEADER,
        "--",
        ".",
        ":(exclude)*.md",
        ":(exclude)LICENSE",
        ":(exclude)LICENSES/**",
        ":(exclude)COPYING*",
    )
    if result.exit_code not in {0, 1}:
        raise TaskError(f"{TASK}: git grep failed with exit code {result.exit_code}")

    invalid: list[str] = []
    for line in result.output.splitlines():
        match = re.search(r":([0-9]+):", line)
        if match is None or int(match.group(1)) > 10:
            continue
        text = line[match.end() :]
        reason = ""
        if LEGACY_COPYRIGHT.match(text):
            reason = "legacy copyright header"
        elif (position := text.find("SPDX-FileCopyrightText:")) >= 0:
            suffix = text[position + len("SPDX-FileCopyrightText:") :]
            if (
                not suffix
                or not suffix.startswith(" ")
                or (len(suffix) > 1 and suffix[1].isspace())
            ):
                reason = "expected exactly one space after colon"
        if (
            not reason
            and "SPDX-FileCopyrightText:" in text
            and "Nikolay Govorov" in text
            and not CANONICAL_COPYRIGHT.search(text)
        ):
            reason = "expected 2026 Nikolay Govorov"
        if reason:
            invalid.append(f"{reason}: {line}")

    if invalid:
        print("Invalid copyright headers:", *invalid, sep="\n", file=sys.stderr)
        raise TaskError(f"{TASK}: invalid copyright headers")


async def main(args: Sequence[str]) -> None:
    argparse.ArgumentParser(prog="mise run licenses --").parse_args(args)
    await check_copyright_headers()
    await run("reuse", "lint")
    if Path("Cargo.toml").is_file():
        await run("cargo-deny", "check")


if __name__ == "__main__":
    task_main(TASK, main, sys.argv[1:])
