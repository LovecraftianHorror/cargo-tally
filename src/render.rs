use crate::total::Total;
use anyhow::Result;
use cargo_tally::matrix::Matrix;
use cargo_tally::timestamp::NaiveDateTime;
use std::env;
use std::fmt::{self, Display};
use std::fs;
use std::path::PathBuf;

pub(crate) fn graph(
    title: Option<&str>,
    transitive: bool,
    results: &Matrix,
    labels: &[String],
    total: Option<&Total>,
) -> Result<PathBuf> {
    let now = NaiveDateTime::now();

    let title = if let Some(title) = title {
        title
    } else if total.is_some() {
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
    for (i, label) in labels.iter().enumerate() {
        data += "      {\"name\":\"";
        data += label;
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

    let mut html = include_str!("index.html")
        .replace("var title = \"\";", &format!("var title = \"{}\";", title))
        .replace("var data = [];", &data);

    if total.is_none() {
        html = html.replace(
            "var yFormatter = d3.format(\".\" + Math.max(0, d3.precisionFixed(stepSize) - 2) + \"%\");",
            "var yFormatter = d3.format(\",\");"
        );
        html = html.replace(
            "var tooltipFormatter = d3.format(\".\" + Math.max(0, d3.precisionFixed(stepSize / 10) - 2) + \"%\");",
            "var tooltipFormatter = d3.format(\",\");",
        );
    }

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
        write!(formatter, "{}", self.0.millis())?;
        formatter.write_str(", \"edges\":")?;
        if let Some(total) = self.2 {
            let total = total.eval(self.0);
            if total == 0 {
                formatter.write_str("0")?;
            } else {
                let fraction = self.1 as f32 / total as f32;
                write!(formatter, "{}", fraction)?;
            }
        } else {
            write!(formatter, "{}", self.1)?;
        }
        formatter.write_str("},\n")?;
        Ok(())
    }
}
