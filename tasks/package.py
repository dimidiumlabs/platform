#!/usr/bin/env -S pipx run --backend pip
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: 0BSD
# fmt: off
#MISE description="Build release archives and signed Linux packages"
#MISE tools={"pipx"="1.16.7","python"="3.14.7","nfpm"="2.47.0"}
# fmt: on
# /// script
# requires-python = ">=3.11"
# dependencies = ["shellous==0.42.0"]
# ///

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
import tempfile
from collections.abc import Sequence
from pathlib import Path

sys.dont_write_bytecode = True

from libs.common import (
    APKSigning,
    GPGSigning,
    TaskError,
    require_command,
    run,
    task_main,
)

TASK = "package"
SYSTEM_FORMATS = {"deb", "rpm", "apk"}
ARCHIVE_FORMATS = {"tar.gz", "zip"}
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SAFE_KEY_VERSION = re.compile(r"^[0-9]{4}$")


def validate(arguments: argparse.Namespace, command: argparse.ArgumentParser) -> None:
    formats = set(arguments.formats)
    if formats & SYSTEM_FORMATS:
        if not arguments.version or not arguments.arch:
            command.error("--version and --arch are required for deb, rpm, and apk")
        if not arguments.config.is_file():
            raise TaskError(f"{TASK}: {arguments.config} not found")
    if formats & ARCHIVE_FORMATS:
        if arguments.archive_root is None or not arguments.archive_name:
            command.error(
                "--archive-root and --archive-name are required for tar.gz and zip"
            )
        if not arguments.archive_root.is_dir():
            raise TaskError(f"{TASK}: {arguments.archive_root} not found")
        if not SAFE_NAME.fullmatch(arguments.archive_name):
            raise TaskError(f"{TASK}: invalid archive name: {arguments.archive_name}")
    if arguments.apk_public_key and not SAFE_NAME.fullmatch(arguments.apk_public_key):
        raise TaskError(
            f"{TASK}: invalid APK public key name: {arguments.apk_public_key}"
        )


async def create_archive(
    archive_format: str,
    output: Path,
    root: Path,
    name: str,
) -> None:
    if archive_format == "tar.gz":
        require_command("tar", TASK)
        await run("tar", "-czf", output / f"{name}.tar.gz", "-C", root, ".")
        return
    require_command("zip", TASK)
    destination = output / f"{name}.zip"
    destination.unlink(missing_ok=True)
    await run.set(cwd=root)("zip", "-qry", destination, ".")


async def sign_package(
    package_format: str, package: Path, signing: GPGSigning | None
) -> None:
    if signing is None or package_format not in {"deb", "rpm"}:
        return
    command = run.set(env=signing.environment, inherit_env=False)
    if package_format == "deb":
        require_command("debsigs", TASK)
        await command(
            "debsigs",
            "--sign=origin",
            f"--default-key={signing.key_id}",
            package,
        )
    else:
        require_command("rpmsign", TASK)
        await command(
            "rpmsign",
            "--define",
            f"_gpg_name {signing.key_id}",
            "--addsign",
            package,
        )


async def main(args: Sequence[str]) -> None:
    command = argparse.ArgumentParser(
        prog="mise run package --",
        usage=(
            "%(prog)s --output DIR [--config FILE] "
            "[--version VERSION --arch ARCH] [--apk-public-key FILE] "
            "[--archive-root DIR --archive-name NAME] "
            "deb|rpm|apk|tar.gz|zip..."
        ),
    )
    command.add_argument("--config", default="nfpm.yaml", type=Path)
    command.add_argument("--version")
    command.add_argument("--arch")
    command.add_argument("--output", required=True, type=Path)
    command.add_argument("--archive-root", type=Path)
    command.add_argument("--archive-name")
    command.add_argument("--apk-public-key")
    command.add_argument(
        "formats", nargs="+", choices=sorted(SYSTEM_FORMATS | ARCHIVE_FORMATS)
    )
    arguments = command.parse_args(args)
    validate(arguments, command)

    arguments.output.mkdir(parents=True, exist_ok=True)
    output = arguments.output.resolve()
    formats = set(arguments.formats)

    with tempfile.TemporaryDirectory(prefix="package-") as directory:
        work = Path(directory)
        environment = dict(os.environ)
        for name in (
            "GPG_PRIVATE_KEY",
            "APK_PRIVATE_KEY",
            "SIGNING_PRIVATE_KEY",
            "NFPM_PASSPHRASE",
            "NFPM_DEB_PASSPHRASE",
            "NFPM_RPM_PASSPHRASE",
        ):
            environment.pop(name, None)
        config = arguments.config
        if "apk" in formats and "${PACKAGE_KEY_VERSION}" in config.read_text():
            key_version = os.environ.get("PACKAGE_KEY_VERSION", "")
            if not SAFE_KEY_VERSION.fullmatch(key_version):
                raise TaskError(
                    f"{TASK}: invalid PACKAGE_KEY_VERSION: {key_version or '<empty>'}"
                )
            config = work / "nfpm.yaml"
            config.write_text(
                arguments.config.read_text().replace(
                    "${PACKAGE_KEY_VERSION}", key_version
                )
            )

        gpg_signing: GPGSigning | None = None
        if formats & {"deb", "rpm"} and os.environ.get("GPG_PRIVATE_KEY"):
            gpg_signing = await GPGSigning.create(TASK, work)
            await gpg_signing.prime_agent()
            environment = gpg_signing.package_environment()

        apk_signing: APKSigning | None = None
        if "apk" in formats and os.environ.get("APK_PRIVATE_KEY"):
            apk_signing = APKSigning(TASK, work)
            environment["APK_SIGNING_KEY"] = str(apk_signing.private_key_file)
        elif os.environ.get("APK_SIGNING_KEY"):
            environment["APK_SIGNING_KEY"] = os.environ["APK_SIGNING_KEY"]

        if "apk" in formats and arguments.apk_public_key:
            public_key = output / arguments.apk_public_key
            if apk_signing is not None:
                await apk_signing.export_public_key(public_key)
            elif environment.get("APK_SIGNING_KEY"):
                require_command("openssl", TASK)
                await run.set(env=environment, inherit_env=False)(
                    "openssl",
                    "rsa",
                    "-in",
                    environment["APK_SIGNING_KEY"],
                    "-pubout",
                    "-out",
                    public_key,
                )

        for package_format in arguments.formats:
            if package_format in SYSTEM_FORMATS:
                require_command("nfpm", TASK)
                package_environment = dict(environment)
                package_environment.update(
                    ARCH=arguments.arch,
                    VERSION=arguments.version,
                )
                package_output = work / f"output-{package_format}"
                package_output.mkdir()
                await run.set(env=package_environment, inherit_env=False)(
                    "nfpm",
                    "package",
                    "--config",
                    config,
                    "--packager",
                    package_format,
                    "--target",
                    f"{package_output}/",
                )
                packages = list(package_output.iterdir())
                if len(packages) != 1 or not packages[0].is_file():
                    raise TaskError(
                        f"{TASK}: nFPM produced an unexpected number of packages"
                    )
                await sign_package(package_format, packages[0], gpg_signing)
                shutil.move(packages[0], output / packages[0].name)
            else:
                await create_archive(
                    package_format,
                    output,
                    arguments.archive_root,
                    arguments.archive_name,
                )


if __name__ == "__main__":
    task_main(TASK, main, sys.argv[1:])
