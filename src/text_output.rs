use std::{collections::BTreeMap, io::Write};

use crate::{analyzer::Analyzer, bus_usage::BusUsage};

fn generate_tabled<O>(header: &Vec<String>, data: &Vec<Vec<String>>, style: O) -> tabled::Table
where
    O: tabled::settings::TableOption<
            tabled::grid::records::vec_records::VecRecords<
                tabled::grid::records::vec_records::Text<String>,
            >,
            tabled::grid::config::ColoredConfig,
            tabled::grid::dimension::CompleteDimension,
        >,
{
    let mut builder = tabled::builder::Builder::new();
    builder.push_record(header);
    for u in data {
        builder.push_record(u);
    }
    let mut t = builder.build();
    t.with(style);
    t
}

fn get_header(usages: &[&BusUsage]) -> Vec<String> {
    if usages.is_empty() {
        return vec![];
    }
    let mut header = vec![String::from("bus name")];
    let stats = usages
        .iter()
        .map(|u| u.get_statistics())
        .collect::<Vec<_>>();
    for stat in 0..stats[0].len() {
        match &stats[0][stat] {
            crate::bus_usage::Statistic::Percentage(percentage_statistic) => percentage_statistic
                .data_labels
                .iter()
                .map(|(_, l)| l)
                .for_each(|l| header.push((*l).to_owned())),
            crate::bus_usage::Statistic::Bucket(buckets_statistic) => {
                header.push(buckets_statistic.name.to_owned())
            }
            crate::bus_usage::Statistic::Timeline(timeline_statistic) => {
                header.push(timeline_statistic.name.to_owned())
            }
        }
    }
    header
}

fn get_data(usages: &[&BusUsage], verbose: bool) -> Vec<Vec<String>> {
    usages
        .iter()
        .map(|u| {
            let mut v = vec![u.get_name().to_owned()];
            for s in u.get_statistics().iter() {
                match s {
                    crate::bus_usage::Statistic::Percentage(percentage_statistic) => {
                        for (d, _) in percentage_statistic.data_labels.iter() {
                            v.push(d.to_string());
                        }
                    }
                    crate::bus_usage::Statistic::Bucket(buckets_statistic) => {
                        if verbose {
                            v.push(format!("{:?}", buckets_statistic.data));
                        } else {
                            let buckets: BTreeMap<_, _> =
                                buckets_statistic.get_buckets().into_iter().collect();
                            let d = buckets
                                .iter()
                                .filter_map(|(&i, v)| {
                                    if *v > 0 {
                                        Some(if i < 2 {
                                            format!("{} x{}", i, *v)
                                        } else if i >= 41 {
                                            format!("2^{}+ x{}", i, *v)
                                        } else if i >= 21 {
                                            let i = i as u32 - 20;
                                            format!("{}-{}M x{}", 1 << (i - 1), 1 << i, *v)
                                        } else if i >= 11 {
                                            let i = i as u32 - 10;
                                            format!("{}-{}k x{}", 1 << (i - 1), 1 << i, *v)
                                        } else {
                                            format!(
                                                "{}-{} x{}",
                                                1 << (i as u64 - 1),
                                                (1 << i as u64) - 1,
                                                *v
                                            )
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("; ");
                            if d.is_empty() {
                                v.push("No transaction on this bus".into())
                            } else {
                                v.push(d);
                            }
                        }
                    }
                    crate::bus_usage::Statistic::Timeline(timeline_statistic) => {
                        v.push(timeline_statistic.display.clone());
                    }
                }
            }
            v
        })
        .collect::<Vec<_>>()
}

fn print_statistics_internal<O>(
    write: &mut impl Write,
    analyzers: &[Box<dyn Analyzer>],
    verbose: bool,
    style: O,
) where
    O: tabled::settings::TableOption<
            tabled::grid::records::vec_records::VecRecords<
                tabled::grid::records::vec_records::Text<String>,
            >,
            tabled::grid::config::ColoredConfig,
            tabled::grid::dimension::CompleteDimension,
        > + Clone,
{
    let usages = analyzers
        .iter()
        .map(|a| a.get_results().expect("Already calculated"))
        .collect::<Vec<_>>();
    let single_usages = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::SingleChannel(_) => Some(u),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !single_usages.is_empty() {
        let header = get_header(&single_usages);
        let data = get_data(&single_usages, verbose);
        writeln!(write, "{}", generate_tabled(&header, &data, style.clone())).unwrap();
    }

    let multi_usage: Vec<_> = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::MultiChannel(_) => Some(u),
            _ => None,
        })
        .collect();
    if !multi_usage.is_empty() {
        let header = get_header(&multi_usage);
        let data = get_data(&multi_usage, verbose);
        writeln!(write, "{}", generate_tabled(&header, &data, style)).unwrap();
    }
}

pub fn print_statistics(write: &mut impl Write, analyzers: &[Box<dyn Analyzer>], verbose: bool) {
    print_statistics_internal(
        write,
        analyzers,
        verbose,
        tabled::settings::Style::rounded(),
    );
}

pub fn generate_md_table(write: &mut impl Write, analyzers: &[Box<dyn Analyzer>], verbose: bool) {
    print_statistics_internal(
        write,
        analyzers,
        verbose,
        tabled::settings::Style::markdown(),
    );
}

pub fn generate_csv(write: &mut impl Write, analyzers: &[Box<dyn Analyzer>], verbose: bool) {
    let usages = analyzers
        .iter()
        .map(|a| a.get_results().expect("Already calculated"))
        .collect::<Vec<_>>();
    let mut wtr = csv::Writer::from_writer(write);
    let single_usages: Vec<_> = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::SingleChannel(_) => Some(u),
            _ => None,
        })
        .collect();
    if !single_usages.is_empty() {
        let header = get_header(&single_usages);
        wtr.write_record(header).unwrap();
        let data = get_data(&single_usages, verbose);
        for d in data {
            wtr.write_record(d).unwrap();
        }
    }
    let multi_usage: Vec<_> = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::MultiChannel(_) => Some(u),
            _ => None,
        })
        .collect();
    if !multi_usage.is_empty() {
        let header = get_header(&multi_usage);
        wtr.write_record(header).unwrap();
        let data = get_data(&multi_usage, verbose);
        for d in data {
            wtr.write_record(d).unwrap();
        }
    }
    wtr.flush().unwrap();
}
