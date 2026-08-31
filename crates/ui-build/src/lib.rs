// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: Apache-2.0

mod css;
mod script;

use std::{
    collections::BTreeMap,
    env, fmt, fs,
    path::{Path, PathBuf},
};

/// Compiles the calling package's styles into its `OUT_DIR`.
///
/// This reads `CARGO_MANIFEST_DIR` and `OUT_DIR`, as supplied by Cargo to build scripts, writes
/// `stylesheet.css`, `css_modules.rs`, and `script.js`, and emits the required Cargo rerun directives.
///
/// # Errors
/// Returns an error when Cargo has not supplied its build-script environment, an input cannot be
/// read, or CSS compilation fails.
pub fn build() -> Result<(), Error> {
    let manifest_dir = env_path("CARGO_MANIFEST_DIR")?;
    let out_dir = env_path("OUT_DIR")?;
    compile(&manifest_dir, &out_dir).map(|_| ())
}

/// The results produced by [`compile`]. This is mainly useful for build-tool integration tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Input paths relative to the service manifest, in compilation order.
    pub inputs: Vec<String>,
    /// Cargo directives a build script must print.
    pub rerun_if_changed: Vec<String>,
}

/// Compiles styles in `manifest_dir` into `out_dir` without modifying source files.
///
/// Build scripts normally use [`build`]. This function also prints [`Output::rerun_if_changed`].
///
/// # Errors
/// Returns an error when inputs cannot be discovered or read, names are ambiguous, imports or
/// composition need another file, or Lightning CSS rejects an input.
pub fn compile(manifest_dir: &Path, out_dir: &Path) -> Result<Output, Error> {
    let inputs = discover(manifest_dir)?;
    let mut stylesheet = String::new();
    let mut modules = BTreeMap::new();
    let mut scripts = Vec::new();

    for input in &inputs {
        let source = fs::read_to_string(&input.path).map_err(|source| Error::Io {
            path: input.path.clone(),
            source,
        })?;
        match input.kind {
            InputKind::ModuleCss => {
                let (code, exports) = css::compile_module(&source, &input.logical_path)?;
                stylesheet.push_str(&code);
                let module_name = input.module_name.clone().ok_or_else(|| Error::Css {
                    path: input.logical_path.clone(),
                    message: "module input has no module name".to_owned(),
                })?;
                modules.insert(module_name, exports);
            }
            InputKind::GlobalCss => {
                stylesheet.push_str(&css::compile_global(&source, &input.logical_path)?);
            }
            InputKind::Script => scripts.push((
                input.logical_path.as_str(),
                source,
                input
                    .path
                    .extension()
                    .is_some_and(|extension| extension == "ts"),
            )),
        }
    }
    let script = script::compile_scripts(&scripts)?;

    fs::create_dir_all(out_dir).map_err(|source| Error::Io {
        path: out_dir.to_owned(),
        source,
    })?;
    write_output(&out_dir.join("stylesheet.css"), &stylesheet)?;
    write_output(
        &out_dir.join("css_modules.rs"),
        &css::render_bindings(&modules),
    )?;
    write_output(&out_dir.join("script.js"), &script)?;

    let mut rerun_if_changed = vec!["src".to_owned()];
    rerun_if_changed.extend(inputs.iter().map(|input| input.logical_path.clone()));
    for directive in &rerun_if_changed {
        println!("cargo:rerun-if-changed={directive}");
    }
    Ok(Output {
        inputs: inputs.into_iter().map(|input| input.logical_path).collect(),
        rerun_if_changed,
    })
}

/// Errors returned while discovering or compiling build-script inputs.
#[derive(Debug)]
pub enum Error {
    MissingEnvironment(&'static str),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Css {
        path: String,
        message: String,
    },
    JavaScript {
        path: String,
        message: String,
    },
    ModuleSyntax {
        path: String,
    },
    DuplicateModule {
        name: String,
        paths: Vec<String>,
    },
    CrossFileComposes {
        path: String,
    },
    Import {
        path: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnvironment(name) => {
                write!(f, "missing build environment variable {name}")
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Css { path, message } | Self::JavaScript { path, message } => {
                write!(f, "{path}: {message}")
            }
            Self::ModuleSyntax { path } => write!(
                f,
                "{path}: ES module syntax is unsupported in classic scripts"
            ),
            Self::DuplicateModule { name, paths } => write!(
                f,
                "CSS module basename {name:?} is ambiguous: {}",
                paths.join(", ")
            ),
            Self::CrossFileComposes { path } => {
                write!(f, "{path}: composing from another file is unsupported")
            }
            Self::Import { path } => write!(f, "{path}: stylesheet imports are unsupported"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug)]
enum InputKind {
    GlobalCss,
    ModuleCss,
    Script,
}

#[derive(Debug)]
struct Input {
    path: PathBuf,
    logical_path: String,
    kind: InputKind,
    module_name: Option<String>,
}

fn env_path(name: &'static str) -> Result<PathBuf, Error> {
    env::var_os(name)
        .map(PathBuf::from)
        .ok_or(Error::MissingEnvironment(name))
}

fn discover(manifest_dir: &Path) -> Result<Vec<Input>, Error> {
    let src = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_inputs(&src, &mut files)?;
    files.sort();
    let mut basenames: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut inputs = Vec::new();
    for path in files {
        let logical_path = path
            .strip_prefix(manifest_dir)
            .map_err(|_| Error::Css {
                path: path.display().to_string(),
                message: "input is outside the manifest directory".to_owned(),
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let kind = classify_input(&path, &logical_path);
        let Some(kind) = kind else { continue };
        let module_name = matches!(kind, InputKind::ModuleCss).then(|| {
            css::rust_identifier(
                path.file_stem()
                    .and_then(|stem| Path::new(stem).file_stem())
                    .and_then(|stem| stem.to_str())
                    .expect("CSS module file name"),
            )
        });
        if let Some(name) = &module_name {
            basenames
                .entry(name.clone())
                .or_default()
                .push(logical_path.clone());
        }
        inputs.push(Input {
            path,
            logical_path,
            kind,
            module_name,
        });
    }
    for (name, paths) in basenames {
        if paths.len() > 1 {
            return Err(Error::DuplicateModule { name, paths });
        }
    }
    Ok(inputs)
}

fn classify_input(path: &Path, logical_path: &str) -> Option<InputKind> {
    let extension = path.extension()?.to_str()?;
    match extension {
        "css" if path.file_stem()?.to_str()?.ends_with(".module") => Some(InputKind::ModuleCss),
        "css"
            if logical_path
                .strip_prefix("src/styles/")
                .is_some_and(|relative| !relative.contains('/')) =>
        {
            Some(InputKind::GlobalCss)
        }
        "js" => Some(InputKind::Script),
        "ts" if Path::new(path.file_stem()?).extension().is_none() => Some(InputKind::Script),
        _ => None,
    }
}

fn collect_inputs(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(Error::Io {
                path: directory.to_owned(),
                source,
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.to_owned(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_inputs(&path, files)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("css" | "js" | "ts")
        ) {
            files.push(path);
        }
    }
    Ok(())
}

fn write_output(path: &Path, content: &str) -> Result<(), Error> {
    fs::write(path, content).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_only_supported_inputs_in_logical_order() {
        let project = tempdir().unwrap();
        fs::create_dir_all(project.path().join("src/z")).unwrap();
        fs::create_dir_all(project.path().join("src/styles")).unwrap();
        fs::write(project.path().join("src/z/a.module.css"), ".a {}").unwrap();
        fs::write(project.path().join("src/styles/site.css"), "body {}").unwrap();
        fs::write(project.path().join("src/ignored.css"), ".ignored {}").unwrap();
        fs::write(project.path().join("src/z/first.js"), "window.first = 1;").unwrap();
        fs::write(project.path().join("src/z/second.ts"), "window.second = 2;").unwrap();
        fs::write(
            project.path().join("src/z/types.d.ts"),
            "interface Value {}",
        )
        .unwrap();
        fs::write(
            project.path().join("src/z/view.tsx"),
            "const view = <div />;",
        )
        .unwrap();
        let inputs = discover(project.path()).unwrap();
        let paths = inputs
            .into_iter()
            .map(|input| input.logical_path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "src/styles/site.css",
                "src/z/a.module.css",
                "src/z/first.js",
                "src/z/second.ts"
            ]
        );
        assert!(!paths.contains(&"src/z/types.d.ts".to_owned()));
        assert!(!paths.contains(&"src/z/view.tsx".to_owned()));
    }

    #[test]
    fn rejects_duplicate_modules() {
        let project = tempdir().unwrap();
        fs::create_dir_all(project.path().join("src/a")).unwrap();
        fs::create_dir_all(project.path().join("src/b")).unwrap();
        fs::write(project.path().join("src/a/card.module.css"), ".a {}").unwrap();
        fs::write(project.path().join("src/b/card.module.css"), ".b {}").unwrap();
        assert!(matches!(
            discover(project.path()),
            Err(Error::DuplicateModule { .. })
        ));
    }

    #[test]
    fn output_is_checkout_independent() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        let first_out = tempdir().unwrap();
        let second_out = tempdir().unwrap();
        for project in [&first, &second] {
            fs::create_dir_all(project.path().join("src/components")).unwrap();
            fs::write(
                project.path().join("src/components/card.module.css"),
                ".card { color: red; }",
            )
            .unwrap();
            fs::write(
                project.path().join("src/components/script.js"),
                "window.value = 1;",
            )
            .unwrap();
        }
        compile(first.path(), first_out.path()).unwrap();
        compile(second.path(), second_out.path()).unwrap();
        for output in ["stylesheet.css", "css_modules.rs", "script.js"] {
            assert_eq!(
                fs::read_to_string(first_out.path().join(output)).unwrap(),
                fs::read_to_string(second_out.path().join(output)).unwrap(),
            );
        }
    }

    #[test]
    fn compilation_writes_all_outputs_and_rerun_directives() {
        let project = tempdir().unwrap();
        let output = tempdir().unwrap();
        fs::create_dir_all(project.path().join("src/styles")).unwrap();
        fs::write(
            project.path().join("src/card.module.css"),
            ".card { color: red; }",
        )
        .unwrap();
        fs::write(
            project.path().join("src/styles/site.css"),
            "body { margin: 0px; }",
        )
        .unwrap();
        let result = compile(project.path(), output.path()).unwrap();
        let stylesheet = fs::read_to_string(output.path().join("stylesheet.css")).unwrap();
        let bindings = fs::read_to_string(output.path().join("css_modules.rs")).unwrap();
        assert!(!stylesheet.contains(".card{") && stylesheet.contains("body{margin:0"));
        assert!(bindings.contains("pub mod card"));
        assert_eq!(
            fs::read_to_string(output.path().join("script.js")).unwrap(),
            ""
        );
        assert_eq!(
            result.rerun_if_changed,
            ["src", "src/card.module.css", "src/styles/site.css"]
        );
    }
}
