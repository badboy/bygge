use std::{
    convert::TryFrom,
    fmt,
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{self, Command},
};

use cargo_lock::{Dependency, Lockfile};
use cargo_toml::Manifest;
use petgraph::visit::Bfs;

const DEFAULT_RULES: &str = r#"# Rules generated by bygge. DO NOT MODIFY BY HAND!
extraargs = --cap-lints allow -C debuginfo=2

rule cargo-fetch
  command = cargo fetch --manifest-path $in && touch $out
  description = CARGO $in

rule rustc
  command = rustc --crate-name $name $in --emit=$emit --out-dir $outdir $extraargs $args && sed -i '' '/\.d:/g' $depfile
  description = RUSTC $out
  depfile = $depfile
  deps = gcc

build Cargo.lock: cargo-fetch Cargo.toml
"#;

const REGISTRY_PATH: &str = "/Users/jrediger/.cargo/registry/src/github.com-1ecc6299db9ec823";

struct Error(String);

impl Error {
    fn new<S: Into<String>>(msg: S) -> Error {
        Error(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error: {}", self.0)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

enum Task {
    Build,
    Create,
}

impl TryFrom<&str> for Task {
    type Error = Error;

    fn try_from(cmd: &str) -> Result<Task, Error> {
        match cmd {
            "build" => Ok(Task::Build),
            "create" => Ok(Task::Create),
            _ => Err(Error::new(format!("invalid command: {:?}", cmd))),
        }
    }
}

#[derive(Debug)]
struct Args {
    help: bool,
    version: bool,
    verbose: bool,
    manifest_path: String,
    lockfile: String,
    ninja_file: String,
    command: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();

    // Arguments can be parsed in any order.
    let args = Args {
        help: args.contains(["-h", "--help"]),
        version: args.contains(["-V", "--version"]),
        verbose: args.contains(["-v", "--verbose"]),
        manifest_path: args
            .opt_value_from_str(["-p", "--manifest-path"])?
            .unwrap_or_else(|| "Cargo.toml".into()),
        lockfile: args
            .opt_value_from_str(["-l", "--lockfile"])?
            .unwrap_or_else(|| "Cargo.lock".into()),
        ninja_file: args
            .opt_value_from_str(["-n", "--ninjafile"])?
            .unwrap_or_else(|| "build.ninja".into()),
        command: args.subcommand()?.unwrap_or_else(|| "".into()),
    };

    if args.version {
        println!("bygge v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.help {
        usage();
        return Ok(());
    }

    match Task::try_from(&*args.command) {
        Ok(Task::Create) => create(args)?,
        Ok(Task::Build) => build(args)?,
        Err(e) => {
            println!("{}\n", e);
            usage();
            process::exit(1);
        }
    }

    Ok(())
}

fn create(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("==> Creating build file: {}", args.ninja_file);
    command(
        args.verbose,
        &["cargo", "fetch", "--manifest-path", &args.manifest_path],
    )?;

    let mut rules = File::create(&args.ninja_file)?;
    writeln!(rules, "{}", DEFAULT_RULES)?;

    let lockfile = Lockfile::load(&args.lockfile)?;

    let manifest = Manifest::from_path(&args.manifest_path)?;
    let package = manifest.package.unwrap();
    let pkg_name = package.name;
    println!("==> Package: {}", pkg_name);

    let root_package = lockfile
        .packages
        .iter()
        .find(|pkg| pkg.name.as_str() == pkg_name)
        .unwrap();

    if args.verbose {
        println!("==> Detected {} dependencies.", lockfile.packages.len());
    }

    let tree = lockfile.dependency_tree()?;
    let nodes = tree.nodes();
    let graph = tree.graph();

    let (_, &root_idx) = nodes
        .iter()
        .find(|(dep, _)| dep.matches(root_package))
        .unwrap();

    let mut bfs = Bfs::new(&graph, root_idx);
    while let Some(nx) = bfs.next(&graph) {
        let node = &graph[nx];
        let pkg_name = node.name.as_str();
        let norm_pkg_name = normalize_crate_name(pkg_name);

        // The main target we try to build.
        if nx == root_idx {
            build_rule(
                &rules,
                pkg_name,
                &format!("build/{}", norm_pkg_name),
                &["src/main.rs"],
                &[&args.lockfile],
                "build",
                "bin",
                "2018",
                "dep-info,link",
                &node.dependencies,
            )?;

            writeln!(rules, "default build/{}", norm_pkg_name)?;
        } else {
            // All the dependencies

            if skip_dep(pkg_name) {
                continue;
            }

            let crate_path = Path::new(REGISTRY_PATH).join(&format!(
                "{pkg}-{version}",
                pkg = pkg_name,
                version = node.version
            ));
            let toml_path = crate_path.join("Cargo.toml");
            let mut f = File::open(&toml_path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            let manifest = Manifest::from_slice(&buffer)?;
            let entry = manifest
                .lib
                .and_then(|lib| lib.path)
                .unwrap_or_else(|| "src/lib.rs".into());
            let entry_path = crate_path.join(entry);
            let entry_path = entry_path.display().to_string();

            build_rule(
                &rules,
                pkg_name,
                &format!(
                    "build/deps/lib{pkg}.rlib build/deps/lib{pkg}.rmeta",
                    pkg = norm_pkg_name
                ),
                &[&entry_path],
                &[],
                "build/deps",
                "lib",
                &edition(manifest.package.unwrap().edition),
                "dep-info,metadata,link",
                &node.dependencies,
            )?;
        }
    }

    Ok(())
}

fn build(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    command(args.verbose, &["ninja", "-f", &args.ninja_file])?;

    Ok(())
}

fn command(verbose: bool, cmdline: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("==> Running: {}", cmdline.join(" "));
    }

    Command::new(cmdline[0])
        .args(&cmdline[1..])
        .status()
        .map_err(|_| Error::new(format!("failed to run `{}`", cmdline[0])))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::new(format!("`{}` failed.", cmdline[0])))
            }
        })?;

    Ok(())
}

fn build_rule<W: Write>(
    mut out: W,
    pkg_name: &str,
    target: &str,
    deps: &[&str],
    implicit_deps: &[&str],
    outdir: &str,
    crate_type: &str,
    edition: &str,
    emit: &str,
    dependencies: &[Dependency],
) -> Result<(), Box<dyn std::error::Error>> {
    let norm_pkg_name = normalize_crate_name(pkg_name);

    write!(
        out,
        "build {}: rustc {} | {} ",
        target,
        deps.join(" "),
        implicit_deps.join(" ")
    )?;

    for dep in dependencies {
        if skip_dep(dep.name.as_str()) {
            continue;
        }
        write!(
            out,
            "build/deps/lib{}.rlib ",
            normalize_crate_name(dep.name.as_str())
        )?;
    }

    writeln!(out)?;
    writeln!(out, "  name = {} ", norm_pkg_name)?;
    write!(
        out,
        "  args = --crate-type {} --edition {} -L dependency=build/deps ",
        crate_type, edition,
    )?;

    // We don't handle features yet,
    // so let's hackily add some features to make libc compiled correctly.
    if norm_pkg_name == "libc" {
        write!(
            out,
            r#"--cfg 'feature="default"' --cfg 'feature="extra_traits"' --cfg 'feature="std"' --cfg freebsd11 --cfg libc_priv_mod_use --cfg libc_union --cfg libc_const_size_of --cfg libc_align --cfg libc_core_cvoid --cfg libc_packedN "#
        )?;
    }

    for dep in dependencies {
        if skip_dep(dep.name.as_str()) {
            continue;
        }
        write!(
            out,
            "--extern {}=build/deps/lib{}.rlib ",
            normalize_crate_name(dep.name.as_str()),
            normalize_crate_name(dep.name.as_str())
        )?;
    }
    writeln!(out)?;
    writeln!(out, "  outdir = {}", outdir)?;
    writeln!(out, "  emit = {}", emit)?;
    writeln!(out, "  depfile = {}/{}.d", outdir, norm_pkg_name)?;
    writeln!(out)?;

    Ok(())
}

fn normalize_crate_name(crate_name: &str) -> String {
    crate_name.replace('-', "_")
}

fn edition(ed: cargo_toml::Edition) -> &'static str {
    use cargo_toml::Edition::*;
    match ed {
        E2018 => "2018",
        _ => "2015",
    }
}

fn skip_dep(name: &str) -> bool {
    // Skipping some crates we know we can't build
    name.contains("winapi") || name.contains("redox")
}

fn usage() {
    const USAGE: &str = r#"
USAGE:
    bygge [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -p, --manifest-path  Path to Cargo.toml [default: Cargo.toml]
    -l, --lockfile       Path to Cargo.lock [default: Cargo.lock]
    -n, --ninjafile      Path to build file [default: build.ninja]
    -v, --verbose        Enable verbose output
    -h, --help           Print this help and exit.
    -V, --version        Print version info and exit

Available subcommands:
    build    Run the ninja build.
    create   Create the ninja configuration.
"#;

    println!("bygge v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", USAGE);
}
