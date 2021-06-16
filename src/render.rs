use crate::total::Total;
use anyhow::Result;
use cargo_tally::cratemap::CrateMap;
use cargo_tally::matrix::Matrix;
use cargo_tally::query::Query;
use cargo_tally::timestamp::NaiveDateTime;
use std::env;
use std::fmt::{self, Display};
use std::fs;
use std::path::PathBuf;

pub(crate) fn graph(
    transitive: bool,
    queries: &[Query],
    results: &Matrix,
    crates: &CrateMap,
    total: Option<&Total>,
) -> Result<PathBuf> {
    let now = NaiveDateTime::now();

    let title = if total.is_some() {
        if transitive {
            "fraction of crates.io depending transitively"
        } else {
            "fraction of crates.io depending directly"
        }
    } else {
        if transitive {
            "number of crates depending transitively"
        } else {
            "number of crates depending directly"
        }
    };

    let mut data = String::new();
    data += "var data = [\n";
    for (i, query) in queries.iter().enumerate() {
        data += "      {\"name\":\"";
        data += &query.display(crates).to_string();
        data += "\", \"values\":[\n";
        let mut prev = None;
        for (timestamp, row) in results {
            let value = row[i];
            if prev.is_none() {
                if value == 0 {
                    continue;
                }
                let mut secs = timestamp.seconds();
                if timestamp.subsec_nanos() == 0 {
                    secs = secs.saturating_sub(1);
                }
                let timestamp = NaiveDateTime::from_timestamp(secs, 0);
                data += &Row(timestamp, 0, total).to_string();
            } else if prev == Some(value) {
                continue;
            }
            data += &Row(timestamp, value, total).to_string();
            prev = Some(value);
        }
        let (timestamp, last) = results.iter().next_back().unwrap();
        if timestamp < now {
            data += &Row(now, last[i], total).to_string();
        }
        data += "      ]},\n";
    }
    data += "    ];";

    let html = include_str!("index.html")
        .replace("var title = \"\";", &format!("var title = \"{}\";", title))
        .replace("var data = [];", &data);

    let dir = env::temp_dir().join("cargo-tally");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.html", now.millis()));
    fs::write(&path, html)?;
    Ok(path)
}

struct Row<'a>(NaiveDateTime, u32, Option<&'a Total>);

impl<'a> Display for Row<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("        {\"time\":")?;
        Display::fmt(&self.0.millis(), formatter)?;
        formatter.write_str(", \"edges\":")?;
        if let Some(total) = self.2 {
            let fraction = self.1 as f32 / total.eval(self.0) as f32;
            write_truncated(formatter, fraction)?;
        } else {
            Display::fmt(&self.1, formatter)?;
        }
        formatter.write_str("},\n")?;
        Ok(())
    }
}

fn write_truncated(formatter: &mut fmt::Formatter, fraction: f32) -> fmt::Result {
    let mut repr = fraction.to_string();
    if let Some(first_nonzero) = repr.find(|ch| ch >= '1' && ch <= '9') {
        repr.truncate(first_nonzero + 3);
    }
    formatter.write_str(&repr)
}
