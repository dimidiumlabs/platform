#!/usr/bin/env -S pipx run --backend pip
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: Apache-2.0
# fmt: off
#MISE description="Verify contributor identities and CLA acceptance trailers"
#MISE tools={"pipx"="1.16.7","python"="3.14.7"}
# fmt: on
# /// script
# requires-python = ">=3.11"
# dependencies = ["shellous==0.42.0"]
# ///

from __future__ import annotations

import argparse
import os
import re
import sys
from collections.abc import Sequence
from pathlib import Path

sys.dont_write_bytecode = True

from libs.common import TaskError, capture, task_main
from shellous import sh

TASK = "signoff"


def configured_values(path: Path) -> set[str]:
    return {
        line for line in path.read_text().splitlines() if not re.match(r"^\s*#", line)
    }


def cla_version(document: str) -> tuple[int, str]:
    versions = [
        line.removeprefix("Version ")
        for line in document.splitlines()
        if line.startswith("Version ")
    ]
    return len(versions), versions[0] if len(versions) == 1 else ""


def check_identity(
    identity: str,
    role: str,
    short_sha: str,
    approved_emails: set[str],
    signoffs: set[str],
) -> tuple[bool, bool]:
    match = re.fullmatch(r".*<([^<>]*)>", identity)
    if match is None or not match.group(1):
        print(f"Commit {short_sha} has an invalid {role} identity: {identity}")
        return True, False
    if match.group(1) in approved_emails:
        return False, False
    if identity not in signoffs:
        print(
            f"Commit {short_sha} {role} {identity} is missing a matching Signed-off-by"
        )
        return True, True
    return False, True


async def git(*arguments: str) -> str:
    return await capture("git", arguments)


async def main(args: Sequence[str]) -> None:
    argparse.ArgumentParser(prog="mise run signoff --").parse_args(args)
    root = Path((await git("rev-parse", "--show-toplevel")).strip())
    os.chdir(root)

    task_directory = Path(__file__).resolve().parent
    approved_emails_file = task_directory.parent / "config/signoff-approved-emails"
    unsupported_commits_file = task_directory.parent / "config/cla-unsupported-commits"

    cla_file = Path("CLA.md")
    if not cla_file.is_file():
        raise TaskError("CLA.md is missing")
    version_count, head_version = cla_version(cla_file.read_text())
    if version_count != 1:
        raise TaskError("CLA.md must declare exactly one version")
    if not head_version:
        raise TaskError("CLA.md declares an empty version")
    if not approved_emails_file.is_file():
        raise TaskError(
            f"Approved email configuration is missing: {approved_emails_file}"
        )
    if not unsupported_commits_file.is_file():
        raise TaskError(
            f"Unsupported commit configuration is missing: {unsupported_commits_file}"
        )

    approved_emails = configured_values(approved_emails_file)
    unsupported_commits = configured_values(unsupported_commits_file)
    bad = False

    commits = (await git("log", "--no-merges", "--format=%H")).splitlines()
    for sha in commits:
        short_sha = sha[:8]
        signoffs = set(
            (
                await git(
                    "show",
                    "-s",
                    "--format=%(trailers:key=Signed-off-by,valueonly)",
                    sha,
                )
            ).splitlines()
        )
        requires_cla = False

        author = (await git("show", "-s", "--format=%an <%ae>", sha)).rstrip("\n")
        invalid, required = check_identity(
            author, "author", short_sha, approved_emails, signoffs
        )
        bad |= invalid
        requires_cla |= required

        coauthor_output = (
            await git(
                "show",
                "-s",
                "--format=%(trailers:key=Co-authored-by,valueonly)",
                sha,
            )
        ).rstrip("\n")
        for coauthor in coauthor_output.splitlines() if coauthor_output else ():
            invalid, required = check_identity(
                coauthor, "co-author", short_sha, approved_emails, signoffs
            )
            bad |= invalid
            requires_cla |= required

        if requires_cla and sha not in unsupported_commits:
            result = await capture.result("git", "show", f"{sha}:CLA.md").stderr(
                sh.DEVNULL
            )
            document = result.output if result.exit_code == 0 else ""
            expected_count, expected_version = cla_version(document)
            if expected_count != 1:
                print(
                    f"Commit {short_sha} does not contain a CLA.md with exactly one version"
                )
                bad = True
                continue
            if not expected_version:
                print(f"Commit {short_sha} contains an empty CLA version")
                bad = True
                continue

            commit_version = (
                await git(
                    "show",
                    "-s",
                    "--format=%(trailers:key=CLA-Version,valueonly)",
                    sha,
                )
            ).rstrip("\n")
            if commit_version != expected_version:
                if not commit_version:
                    print(
                        f"Commit {short_sha} is missing CLA-Version: {expected_version}"
                    )
                else:
                    print(
                        f"Commit {short_sha} has invalid CLA-Version: {commit_version}"
                    )
                    print(f"Expected CLA-Version: {expected_version}")
                bad = True

    if bad:
        print(
            "Every non-approved author and co-author must accept the CLA in their commit"
        )
        print("Required trailers:")
        print("  CLA-Version: <version from CLA.md>")
        print("  Signed-off-by: Name <email>")
        print("See CLA.md")
        raise SystemExit(1)

    mailmap = Path(".mailmap")
    if not mailmap.is_file():
        raise TaskError("Contributor registry .mailmap is missing")
    mailmap_lines = [
        line
        for line in mailmap.read_text().splitlines()
        if not re.match(r"^\s*#", line)
    ]
    emails = sorted(
        set((await git("log", "--no-merges", "--format=%ae%n%ce")).splitlines())
    )
    missing = False
    for email in emails:
        if email in approved_emails:
            continue
        if not any(f"<{email}>" in line for line in mailmap_lines):
            print(f"Email <{email}> is not in .mailmap")
            missing = True
    if missing:
        print("All authors and committers must be listed in .mailmap")
        raise SystemExit(1)


if __name__ == "__main__":
    task_main(TASK, main, sys.argv[1:])
