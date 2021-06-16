use crate::cratename;
use cargo_tally::version::{self, VersionReq};
use regex::Regex;
use std::convert::TryFrom;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug)]
pub(crate) struct Opt {
    pub db: PathBuf,
    pub exclude: Vec<Regex>,
    pub jobs: usize,
    pub relative: bool,
    pub transitive: bool,
    pub queries: Vec<String>,
}

type App<'b> = clap::App<'static, 'b>;
type Arg = clap::Arg<'static, 'static>;

const USAGE: &str = "\
    cargo tally [OPTIONS] QUERIES...
    cargo tally serde:1.0 'anyhow:^1.0 + thiserror'";

const TEMPLATE: &str = "\
{bin} {version}
David Tolnay <dtolnay@gmail.com>
https://github.com/dtolnay/cargo-tally

USAGE:
    {usage}

OPTIONS:
{unified}\
";

fn app(jobs_help: &str) -> App {
    let mut app = App::new("cargo-tally")
        .usage(USAGE)
        .template(TEMPLATE)
        .arg(arg_db())
        .arg(arg_exclude())
        .arg(arg_jobs(jobs_help))
        .arg(arg_relative())
        .arg(arg_transitive())
        .arg(arg_queries())
        .help_message("Print help information.")
        .version_message("Print version information.");
    if let Some(version) = option_env!("CARGO_PKG_VERSION") {
        app = app.version(version);
    }
    app
}

const DB: &str = "db";
const EXCLUDE: &str = "exclude";
const JOBS: &str = "jobs";
const RELATIVE: &str = "relative";
const TRANSITIVE: &str = "transitive";
const QUERIES: &str = "queries";

pub(crate) fn parse() -> Opt {
    // | threads | duration | allocated |  peak   |
    // |---------|----------|-----------|---------|
    // |     1   |  38.6 s  |   55.2 GB | 11.0 GB |
    // |     2   |  24.8 s  |   55.4 GB | 10.2 GB |
    // |     4   |  14.2 s  |   55.8 GB |  8.8 GB |
    // |     8   |  12.7 s  |   58.4 GB |  8.3 GB |
    // |    16   |  12.6 s  |   59.2 GB |  8.2 GB |
    // |    32   |  12.8 s  |   63.2 GB |  8.4 GB |
    // |    64   |  14.0 s  |   69.5 GB | 11.1 GB |
    let default_jobs = num_cpus::get().min(32);
    let jobs_help = format!(
        "Number of threads to run differential dataflow [default: {}]",
        default_jobs,
    );

    let mut args: Vec<_> = env::args_os().collect();
    if let Some(first) = args.get_mut(0) {
        *first = OsString::from("cargo-tally");
    }
    if args.get(1).map(OsString::as_os_str) == Some(OsStr::new("tally")) {
        args.remove(1);
    }
    let matches = app(&jobs_help).get_matches_from(args);

    let db = PathBuf::from(matches.value_of_os(DB).unwrap());

    let exclude = matches
        .values_of(EXCLUDE)
        .unwrap_or_default()
        .map(|regex| regex.parse().unwrap())
        .collect();

    let jobs = matches
        .value_of(JOBS)
        .map_or(default_jobs, |jobs| jobs.parse().unwrap());

    let relative = matches.is_present(RELATIVE);
    let transitive = matches.is_present(TRANSITIVE);

    let queries = matches
        .values_of(QUERIES)
        .unwrap()
        .map(str::to_owned)
        .collect();

    Opt {
        db,
        exclude,
        jobs,
        relative,
        transitive,
        queries,
    }
}

fn arg_db() -> Arg {
    Arg::with_name(DB)
        .long(DB)
        .takes_value(true)
        .value_name("PATH")
        .default_value("./db-dump.tar.gz")
        .help("Path to crates.io's database dump")
}

fn arg_exclude() -> Arg {
    Arg::with_name(EXCLUDE)
        .long(EXCLUDE)
        .multiple(true)
        .value_name("REGEX")
        .validator_os(validate_parse::<Regex>)
        .help("Ignore a dependency coming from any crates matching regex")
}

fn arg_jobs<'b>(help: &'b str) -> clap::Arg<'static, 'b> {
    Arg::with_name(JOBS)
        .long(JOBS)
        .short("j")
        .takes_value(true)
        .value_name("N")
        .validator_os(validate_parse::<usize>)
        .help(help)
}

fn arg_relative() -> Arg {
    Arg::with_name(RELATIVE)
        .long(RELATIVE)
        .help("Display as a fraction of total crates, not absolute number")
}

fn arg_transitive() -> Arg {
    Arg::with_name(TRANSITIVE)
        .long(TRANSITIVE)
        .help("Count transitive dependencies, not just direct dependencies")
}

fn arg_queries() -> Arg {
    Arg::with_name(QUERIES)
        .required(true)
        .multiple(true)
        .value_name("QUERIES")
        .validator_os(validate_query)
        .help("Queries")
}

fn validate_utf8(arg: &OsStr) -> Result<&str, OsString> {
    arg.to_str()
        .ok_or_else(|| OsString::from("invalid utf-8 sequence"))
}

fn validate_parse<T>(arg: &OsStr) -> Result<(), OsString>
where
    T: FromStr,
    T::Err: Display,
{
    validate_utf8(arg)?
        .parse::<T>()
        .map(drop)
        .map_err(|err| OsString::from(err.to_string()))
}

fn validate_query(arg: &OsStr) -> Result<(), OsString> {
    for predicate in validate_utf8(arg)?.split('+') {
        let predicate = predicate.trim();

        let (name, req) = if let Some((name, req)) = predicate.split_once(':') {
            (name, Some(req))
        } else {
            (predicate, None)
        };

        if !cratename::valid(name.trim()) {
            return Err(OsString::from("invalid crate name according to crates.io"));
        }

        if let Some(req) = req {
            let req =
                semver::VersionReq::from_str(req).map_err(|err| OsString::from(err.to_string()))?;
            match VersionReq::try_from(req) {
                Ok(_req) => {}
                Err(version::UnsupportedPrerelease) => {
                    return Err(OsString::from("prerelease requirement is not supported"));
                }
            }
        }
    }
    Ok(())
}
