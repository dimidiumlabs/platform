# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: Apache-2.0

from __future__ import annotations

import asyncio
import os
import shutil
import sys
from collections.abc import Awaitable, Callable, Sequence
from pathlib import Path

from shellous import ResultError, sh

run = sh.stdout(sh.INHERIT).stderr(sh.INHERIT)
capture = sh.stderr(sh.INHERIT)


class TaskError(RuntimeError):
    pass


def require_command(name: str, task: str) -> str:
    command = shutil.which(name)
    if command is None:
        raise TaskError(f"{task}: {name} is required")
    return command


def required_env(name: str, task: str, purpose: str = "") -> str:
    value = os.environ.get(name)
    if value:
        return value
    suffix = f" {purpose}" if purpose else ""
    raise TaskError(f"{task}: {name} is required{suffix}")


def task_main(
    task: str,
    main: Callable[[Sequence[str]], Awaitable[None]],
    args: Sequence[str],
) -> None:
    try:
        asyncio.run(main(args))
    except TaskError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1) from None
    except ResultError as error:
        exit_code = error.result.exit_code
        print(f"{task}: command failed with exit code {exit_code}", file=sys.stderr)
        raise SystemExit(exit_code) from None


class GPGSigning:
    def __init__(self, task: str, work: Path):
        self.task = task
        require_command("gpg", task)
        private_key = required_env("GPG_PRIVATE_KEY", task)
        self.passphrase = required_env("GPG_PASSPHRASE", task)
        self.key_id = required_env("GPG_KEY_ID", task)
        self.short_key_id = self.key_id[-16:]
        self.home = work / "gnupg"
        self.home.mkdir(mode=0o700)
        self.private_key_file = work / "signing.asc"
        self.private_key_file.write_text(private_key)
        self.private_key_file.chmod(0o600)
        self.environment = dict(os.environ)
        self.environment["GNUPGHOME"] = str(self.home)
        self.environment.pop("GPG_PRIVATE_KEY", None)
        self.environment.pop("GPG_PASSPHRASE", None)
        self.environment.pop("APK_PRIVATE_KEY", None)

    @classmethod
    async def create(cls, task: str, work: Path) -> GPGSigning:
        signing = cls(task, work)
        command = run.set(env=signing.environment, inherit_env=False)
        await (
            f"{signing.passphrase}\n"
            | command(
                "gpg",
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "--import",
                signing.private_key_file,
            )
        )
        return signing

    def package_environment(self) -> dict[str, str]:
        environment = dict(self.environment)
        environment["GPG_KEY_ID"] = self.short_key_id
        return environment

    async def prime_agent(self) -> None:
        signature = self.private_key_file.with_suffix(".sig")
        await self.sign(signature, "--detach-sign", self.private_key_file)
        signature.unlink()

    async def export_public_key(self, output: Path) -> None:
        command = run.set(env=self.environment, inherit_env=False)
        await command(
            "gpg",
            "--batch",
            "--yes",
            "--armor",
            "--export",
            self.key_id,
        ).stdout(output)

    async def verify_public_bundle(self, bundle: Path) -> None:
        command = capture.set(env=self.environment, inherit_env=False)
        output = await command(
            "gpg",
            "--batch",
            "--with-colons",
            "--show-keys",
            bundle,
        )
        fingerprints = {
            line.split(":")[9]
            for line in output.splitlines()
            if line.startswith("fpr:")
        }
        if self.key_id not in fingerprints:
            raise TaskError(
                f"{self.task}: packages.gpg does not contain signing key {self.key_id}"
            )

    async def sign(self, output: Path, *arguments: str | Path) -> None:
        command = run.set(env=self.environment, inherit_env=False)
        await (
            f"{self.passphrase}\n"
            | command(
                "gpg",
                f"--default-key={self.key_id}",
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "-o",
                output,
                arguments,
            )
        )


class APKSigning:
    def __init__(self, task: str, work: Path, key_name: str = "packages"):
        require_command("openssl", task)
        private_key = required_env("APK_PRIVATE_KEY", task, "for APK signing")
        self.key_name = key_name
        self.public_key_name = f"{key_name}.rsa.pub"
        self.private_key_file = work / f"{key_name}.rsa"
        self.private_key_file.write_text(private_key)
        self.private_key_file.chmod(0o600)
        self.environment = dict(os.environ)
        self.environment.pop("APK_PRIVATE_KEY", None)
        self.environment.pop("GPG_PRIVATE_KEY", None)
        self.environment.pop("GPG_PASSPHRASE", None)
        self.environment["APK_SIGNING_KEY"] = str(self.private_key_file)

    async def export_public_key(self, output: Path) -> None:
        command = run.set(env=self.environment, inherit_env=False)
        await command(
            "openssl",
            "rsa",
            "-in",
            self.private_key_file,
            "-pubout",
            "-out",
            output,
        )
