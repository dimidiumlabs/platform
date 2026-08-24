# Dimidium Labs platform

This repository contains shared building blocks for Dimidium Labs projects:
reusable development and release tasks, common Go and npm libraries, and shared
documentation.

The current executable tasks live in `tasks/`. GitHub Actions is only a runner
for these tasks. Projects include `tasks/` with
[mise remote Git includes](https://mise.jdx.dev/tasks/task-configuration.html#remote-git-includes).
By default, tasks are fetched directly from this public repository over HTTPS:

```console
mise run signoff
mise run licenses
```

Consuming projects pin this repository by commit SHA.

## Packaging

Projects build and stage their own binaries and keep their nFPM configuration.
The shared [`package`](tasks/package) task creates only the formats explicitly
requested by a project: nFPM packages (`deb`, `rpm`, or `apk`) and portable
archives (`tar.gz` or `zip`). APK configurations may use
`${PACKAGE_KEY_VERSION}` in `apk.signature.key_name`; the task renders the
four-digit generation before invoking nFPM. DEB and RPM payloads are built by
nFPM and then signed through `debsigs` and `rpmsign`, allowing CI to use only an
OpenPGP signing subkey while the certification key remains offline. The shared
[`publish`](tasks/publish) task adds
explicitly selected package formats to signed repositories in the organization
package bucket.

```console
mise run package -- \
  --version VERSION --arch ARCH --output DIR \
  [--config nfpm.yaml] [--apk-public-key NAME.rsa.pub] \
  deb rpm apk

mise run package -- \
  --archive-root DIR --archive-name NAME --output DIR \
  tar.gz zip
```

## OCI artifacts

The shared [`container`](tasks/container) task builds one or more tagged OCI
images with Docker Buildx. Registry authentication is deliberately left to the
calling workflow, so the same build can be pushed to GHCR, Cloudflare, or
another OCI registry. The [`chart`](tasks/chart) task strictly lints a Helm
chart, packages an immutable version, and can push it to one or more OCI
repositories.

```console
mise run container -- \
  --context . --file deploy/Dockerfile \
  --platform linux/amd64,linux/arm64 \
  --target site --build-arg APP=site \
  --tag ghcr.io/example/site:1.2.3 \
  --cache-scope site --push

mise run chart -- \
  --chart charts/service --version 1.2.3 --app-version 1.2.3 \
  --output dist/charts --push oci://ghcr.io/example/charts
```

Container tags, chart versions, credentials, and release policy remain owned by
the consuming project. `--provenance false --sbom false` is available for
registries that do not accept OCI attestation indexes. Without `--push` or
`--load`, Buildx only validates and caches the build result.

## Package repositories

Projects publish beneath a service-owned prefix at
`https://pkg.dimidiumlabs.io/<service>/`. Channels are explicit, previously
published package payloads are retained, and an S3 lock serializes metadata
updates for each service/channel.

```console
mise run publish -- \
  --service SERVICE --channel CHANNEL --input DIR \
  deb rpm apk
```

The selected formats map to these layouts:

- APT: `<service>/apt/{dists,pool}/<channel>/`
- RPM: `<service>/rpm/<channel>/`
- APK: `<service>/apk/<channel>/<architecture>/`

APT and RPM metadata refer to the aggregate organization OpenPGP bundle at
`/packages.gpg`. Immutable generation keys live at
`/keys/packages.<version>.gpg` and
`/keys/packages.<version>.rsa.pub`. APK packages and indexes embed
the versioned RSA key name. Public keys are provisioned independently; each
publication checks its signing keys against the selected generation and never
creates or replaces key objects.

Bucket configuration comes from `S3_BUCKET`, `S3_ENDPOINT`, `S3_PUBLIC_URL`,
`S3_ACCESS_KEY_ID`, and `S3_SECRET_ACCESS_KEY`. `PACKAGE_KEY_VERSION` selects
the four-digit key generation. OpenPGP signing uses `GPG_PRIVATE_KEY`,
`GPG_PASSPHRASE`, and `GPG_KEY_ID`; APK index signing uses `APK_PRIVATE_KEY`.

## Tool provisioning

Each project declares its toolchain and standalone CLI dependencies in
`mise.toml`. A fresh checkout is provisioned with one command:

```console
mise bootstrap
```

Shared tasks declare task-specific tools in their `#MISE tools` metadata, so
`mise run` installs the same pinned versions on demand. System libraries that
cannot be installed as portable tools belong in `[bootstrap.packages]`.

## Guardrails

### Licensing policy

`tasks/licenses` runs a pinned REUSE version and verifies the repository's
licensing metadata and canonical SPDX copyright headers. In Rust projects it
also runs a pinned `cargo deny check`.

### Sign-off policy

`tasks/signoff` verifies that:

- authors and co-authors with an email from
  `config/signoff-approved-emails` are trusted without a trailer;
- `CLA.md` declares exactly one version;
- every non-approved author and co-author has a `Signed-off-by` trailer exactly
  matching their commit identity;
- every commit with a non-approved author or co-author has exactly one
  `CLA-Version` trailer matching the version declared by `CLA.md` in that
  commit;
- commits listed in `config/cla-unsupported-commits` retain their
  `Signed-off-by` requirement but are explicitly not treated as covered by a
  versioned CLA;
- every non-approved author and committer email in the complete non-merge
  history is registered in `.mailmap`.

Approved emails and unsupported commits are maintained centrally so a pull
request in a consuming repository cannot grant itself an exemption.

## Contributing

We welcome your contributions, including code, bug reports, ideas, and success
stories.

If you are making a contribution for the first time or from a new email, please
add yourself to the `.mailmap`.

### Signoff

To include your code, we ask that you read and agree to the [CLA](./CLA.md). To
sign, add a `CLA-Version: 1.0` and a `Signed-off-by` trailer to every commit
(`git commit -s --trailer "CLA-Version: 1.0"`). Each commit in a pull request
must carry a valid `Signed-off-by` line matching the commit author. Please use
your real name. We cannot include code from anonymous contributors.

AI agents MUST NOT add Signed-off-by tags. Only humans can legally certify the
Contributor License Agreement.

### AI policy

You may use AI agents when writing code and documentation. AI is not allowed for
media including images, videos, fonts at all. You must fully read, understand,
and cleanup any code generated by the agent. We ask that you disclose the
agent's use and indicate the tool, model, and extent of contribution.

Contributions should include an Assisted-by tag in the following format:
`Assisted-by: AGENT_NAME:MODEL_VERSION [TOOL1] [TOOL2]`, for example:
`Assisted-by: Claude:claude-4.6-opus coccinelle sparse`

Remember, AI agents should make software better, not worse.

## Licensing

Unless noted otherwise, software and configuration are licensed under 0BSD.
Documentation is licensed under CC-BY-4.0.
