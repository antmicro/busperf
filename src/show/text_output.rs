use std::{collections::BTreeMap, error::Error, io::Write};

use libbusperf::bus_usage::{BusUsage, Statistic};

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

fn get_header(usages: &[&BusUsage], skipped_stats: &[String]) -> Vec<String> {
    if usages.is_empty() {
        return vec![];
    }
    let mut header = vec![String::from("bus name")];
    let stats = usages
        .iter()
        .map(|u| u.get_statistics(skipped_stats))
        .collect::<Vec<_>>();
    for stat in &stats[0] {
        match stat {
            Statistic::Percentage(percentage_statistic) => percentage_statistic
                .data_labels
                .iter()
                .map(|(_, l)| l)
                .for_each(|l| header.push((*l).to_owned())),
            Statistic::Bucket(buckets_statistic) => header.push(buckets_statistic.name.to_owned()),
            Statistic::Timeline(timeline_statistic) => {
                header.push(timeline_statistic.name.to_owned())
            }
        }
    }
    header
}

fn get_data(usages: &[&BusUsage], verbose: bool, skipped_stats: &[String]) -> Vec<Vec<String>> {
    usages
        .iter()
        .map(|u| {
            let mut v = vec![u.get_name().to_owned()];
            for s in u.get_statistics(skipped_stats).iter() {
                match s {
                    Statistic::Percentage(percentage_statistic) => {
                        for (d, _) in percentage_statistic.data_labels.iter() {
                            v.push(d.to_string());
                        }
                    }
                    Statistic::Bucket(buckets_statistic) => {
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
                    Statistic::Timeline(timeline_statistic) => {
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
    usages: &[&BusUsage],
    verbose: bool,
    style: O,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>>
where
    O: tabled::settings::TableOption<
            tabled::grid::records::vec_records::VecRecords<
                tabled::grid::records::vec_records::Text<String>,
            >,
            tabled::grid::config::ColoredConfig,
            tabled::grid::dimension::CompleteDimension,
        > + Clone,
{
    let single_usages = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::SingleChannel(_) => Some(u),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !single_usages.is_empty() {
        let header = get_header(&single_usages, skipped_stats);
        let data = get_data(&single_usages, verbose, skipped_stats);
        writeln!(write, "{}", generate_tabled(&header, &data, style.clone()))?;
    }

    let multi_usage: Vec<_> = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::MultiChannel(_) => Some(u),
            _ => None,
        })
        .collect();
    if !multi_usage.is_empty() {
        let header = get_header(&multi_usage, skipped_stats);
        let data = get_data(&multi_usage, verbose, skipped_stats);
        writeln!(write, "{}", generate_tabled(&header, &data, style))?;
    }
    Ok(())
}

pub fn print_statistics(
    write: &mut impl Write,
    usages: &[&BusUsage],
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    print_statistics_internal(
        write,
        usages,
        verbose,
        tabled::settings::Style::rounded(),
        skipped_stats,
    )
}

pub fn generate_md_table(
    write: &mut impl Write,
    usages: &[&BusUsage],
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    print_statistics_internal(
        write,
        usages,
        verbose,
        tabled::settings::Style::markdown(),
        skipped_stats,
    )
}

pub fn generate_csv(
    write: &mut impl Write,
    usages: &[&BusUsage],
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut wtr = csv::Writer::from_writer(write);
    let single_usages: Vec<_> = usages
        .iter()
        .filter_map(|&u| match u {
            BusUsage::SingleChannel(_) => Some(u),
            _ => None,
        })
        .collect();
    if !single_usages.is_empty() {
        let header = get_header(&single_usages, skipped_stats);
        wtr.write_record(header)?;
        let data = get_data(&single_usages, verbose, skipped_stats);
        for d in data {
            wtr.write_record(d)?;
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
        let header = get_header(&multi_usage, skipped_stats);
        wtr.write_record(header)?;
        let data = get_data(&multi_usage, verbose, skipped_stats);
        for d in data {
            wtr.write_record(d)?;
        }
    }
    wtr.flush()?;
    Ok(())
}
