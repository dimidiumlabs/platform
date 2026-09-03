# Dimidium Labs platform

Shared Rust crates for web services and reusable `mise` tasks for building,
publishing, and validating projects.

## Web service

- [`dimidiumlabs-ui`](crates/ui) provides the shared design system: fonts,
  assets, design tokens, UI components, and the ordered transport-agnostic
  `AssetsCatalog`.
- [`dimidiumlabs-ui-build`](crates/ui-build) is a build-script tool for global
  styles, CSS Modules, classic JavaScript or TypeScript component scripts, and
  files under `src/assets`. Static files are copied unchanged into build output;
  one generated asset array embeds every resource with its logical name, cache
  policy, bytes, and build-time SHA-384 integrity. Its `build(id, sources,
  assets)` API takes package-local paths below `src` instead of assuming a crate
  layout. Compiled CSS and JavaScript also receive a complete 16-hex xxHash64
  filename fingerprint.
- [`dimidiumlabs-server`](crates/server) serves that catalog through Axum and
  applies CSP, integrity-based strong ETags, conditional request, HEAD, and cache policy.
  Its `service` module provides composable Tower admission, client-IP, host,
  rate-limit, drain, HTML, asset, HSTS, body, and redirect primitives. Top-level
  TLS and Hyper transport modules provide connection-level infrastructure.
  Root resources use their conventional paths; other assets are served from
  `/-/assets/`.

Services keep their pages and component-specific assets in their own crate and
compose `AssetsCatalog::new().with(FOUNDATION).with(APPLICATION)` once for the
HTML document, policy layers, and serving adapter.

## Scripts

Executable tasks live in [`tasks/`](tasks). Consuming projects include this
directory with
[mise remote Git includes](https://mise.jdx.dev/tasks/task-configuration.html#remote-git-includes)
and pin the repository to a commit SHA. Task metadata installs pinned tools on
demand; project toolchains and system packages remain in the consuming project's
`mise.toml`.

Run `mise run <task> -- --help` for the complete command-line interface.

### `package`

Builds only the requested nFPM packages (`deb`, `rpm`, `apk`) or portable
archives (`tar.gz`, `zip`). The consuming project owns its nFPM configuration
and staged files. DEB and RPM packages can be signed with the OpenPGP
environment variables; APK supports `PACKAGE_KEY_VERSION` and APK signing keys.

```console
mise run package -- --version VERSION --arch ARCH --output DIR deb rpm apk
mise run package -- --archive-root DIR --archive-name NAME --output DIR tar.gz zip
```

### `publish`

Publishes selected package formats to the service and channel under
`https://pkg.dimidiumlabs.io/<service>/`. Existing payloads are retained, and an
S3 lock serializes repository metadata updates.

```console
mise run publish -- --service SERVICE --channel CHANNEL --input DIR deb rpm apk
```

Storage uses `S3_BUCKET`, `S3_ENDPOINT`, `S3_PUBLIC_URL`, `S3_ACCESS_KEY_ID`,
and `S3_SECRET_ACCESS_KEY`. Signing uses `PACKAGE_KEY_VERSION`, the `GPG_*`
variables, and `APK_PRIVATE_KEY`.

### `container`

Builds one or more tagged OCI images with Docker Buildx. Authentication and
release policy stay with the caller; use `--push` or `--load` to export the
result.

```console
mise run container -- --context . --file Dockerfile --platform linux/amd64,linux/arm64 --tag REGISTRY/IMAGE:TAG --push
```

### `chart`

Strictly lints a Helm chart, packages an immutable version, and optionally
pushes it to one or more OCI repositories. Use `--lint-only` when no package is
needed.

```console
mise run chart -- --chart charts/service --version VERSION --app-version VERSION --output dist/charts --push oci://REGISTRY/charts
```

### `licenses`

Checks canonical SPDX headers and REUSE metadata. Rust repositories are also
checked with `cargo deny`.

```console
mise run licenses
```

### `licenses-json`

Generates a deterministic JSON bundle of Rust dependency licenses. Repeat
`--target` for all supported targets; `--check` verifies a committed bundle, and
`--offline` uses cached dependency sources. Licenses for bundled non-Rust
resources can be declared in `package.metadata.dimidiumlabs.bundled-licenses`
with an SPDX ID, name, and path relative to `Cargo.toml`.

```console
mise run licenses-json -- --manifest-path Cargo.toml --output licenses.json --target x86_64-unknown-linux-gnu
```

### `signoff`

Checks contributor identities, matching `Signed-off-by` trailers, the
`CLA-Version` declared by each commit's `CLA.md`, and registration of author and
committer addresses in `.mailmap`.

```console
mise run signoff
```

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

Unless noted otherwise, software and configuration are licensed under
Apache-2.0. Documentation is licensed under CC-BY-4.0.
