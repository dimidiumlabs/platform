# Packages and releases

Projects build and stage binaries themselves. Infra only packages and
publishes them.

## Versions

| Channel | Version | Git tag |
| --- | --- | --- |
| nightly | `<base>-nightly.<sequence>.g<short-sha>` | mutable `nightly` |
| stable | `<base>` | `v<base>` |

`base` is `MAJOR.MINOR.PATCH`.

| Format | Nightly | Stable |
| --- | --- | --- |
| DEB | `<base>~nightly.<sequence>+<short-sha>-1` | `<base>-1` |
| RPM | `<base>-0.nightly.<sequence>.<short-sha>` | `<base>-1` |
| APK | `<base>_pre<sequence>~<short-sha>-r0` | `<base>-r0` |

`package` uses the project-owned `nfpm.yaml`:

```console
mise run package -- \
  --version VERSION --arch ARCH --output DIR \
  [--config nfpm.yaml] deb rpm apk
```

`nfpm.yaml` reads `ARCH`, `VERSION`, and `RELEASE`. Signing uses
`GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`, `GPG_KEY_ID`, and `APK_PRIVATE_KEY`.

`publish` updates APT, RPM, and APK repositories on S3:

```console
mise run publish -- \
  --name PROJECT \
  --repository FULL_REPOSITORY_URL \
  --version VERSION \
  --input DIR \
  --s3-bucket BUCKET \
  --s3-endpoint URL \
  --s3-public-url URL \
  [--s3-provider PROVIDER] [--s3-region REGION] \
  [--github-release]
```

The repository is always passed as a full URL and is only interpreted when
`--github-release` is enabled. S3 credentials use `S3_ACCESS_KEY_ID` and
`S3_SECRET_ACCESS_KEY`; signing uses the same GPG variables as `package`.
