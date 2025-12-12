use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    iter::Peekable,
};

use constcat::concat_slices;
use wellen::{Signal, SignalValue, TimeTable, TimeTableIdx};

use crate::analyze::bus::SignalPathFromYaml;
use crate::analyze::{
    analyzer::private::AnalyzerInternal,
    bus::{
        BusCommon, BusDescription, SignalPath, ValueType, axi::AXIBus, get_value, is_value_of_type,
    },
};
use libbusperf::CyclesNum;
use libbusperf::bus_usage::{BusUsage, MultiChannelBusUsage};

use super::Analyzer;

struct AXIFullRd {
    ar_id: SignalPath,
    r_id: SignalPath,
    r_last: SignalPath,
}

pub struct AXIRdAnalyzer {
    common: BusCommon,
    ar: AXIBus,
    r: AXIBus,
    r_resp: SignalPath,
    /// full is optional, if it's None we assume AXI-Lite
    full: Option<AXIFullRd>,
    result: Option<BusUsage>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

struct AXIFullWr {
    aw_id: SignalPath,
    w_last: SignalPath,
    b_id: SignalPath,
}

pub struct AXIWrAnalyzer {
    common: BusCommon,
    aw: AXIBus,
    w: AXIBus,
    b: AXIBus,
    b_resp: SignalPath,
    /// full is optional, if it's None we assume AXI-Lite
    full: Option<AXIFullWr>,
    result: Option<BusUsage>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

const AXI_RD_YAML: &[&str] = concat_slices!([&str]: super::COMMON_YAML, &[
    "ar.id", "ar.ready", "ar.valid",
    "r.id", "r.ready", "r.valid", "r.resp", "r.last",
]);

const AXI_WR_YAML: &[&str] = concat_slices!([&str]: super::COMMON_YAML, &[
    "aw.id", "aw.ready", "aw.valid",
    "w.ready", "w.valid", "w.last",
    "b.ready", "b.valid", "b.resp", "b.id"
]);

// Count how many clock cycles was reset active
fn count_reset(rst: &Signal, active_value: ValueType, start: u32, end: u32) -> u32 {
    let mut last = start;
    let mut reset = 0;
    for (time, value) in rst.iter_changes().filter(|&(t, _)| t > start && t < end) {
        if is_value_of_type(value, active_value) {
            last = time;
        } else {
            reset += time - last;
        }
    }
    reset / 2
}

#[inline]
fn get_id_value(signal: &Signal, time: TimeTableIdx) -> Option<String> {
    get_value_at_time(signal, time.saturating_sub(1))?.to_bit_string()
}

#[inline]
fn get_logic_value(signal: &Signal, time: TimeTableIdx) -> Option<ValueType> {
    get_value(get_value_at_time(signal, time.saturating_sub(1))?)
}

#[inline]
fn get_value_at_time(signal: &Signal, time: TimeTableIdx) -> Option<SignalValue<'_>> {
    Some(signal.get_value_at(&signal.get_offset(time)?, 0))
}

struct Transaction {
    start: TimeTableIdx,
    first_data: Option<TimeTableIdx>,
    last_data: Option<TimeTableIdx>,
    next: TimeTableIdx,
}

impl Transaction {
    fn new(start: TimeTableIdx, next: TimeTableIdx) -> Self {
        Self {
            start,
            first_data: None,
            last_data: None,
            next,
        }
    }
}

impl AXIRdAnalyzer {
    pub fn build_from_yaml(
        yaml: (yaml_rust2::Yaml, yaml_rust2::Yaml),
        default_max_burst_delay: CyclesNum,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Self, Box<dyn Error>> {
        let (name, dict) = yaml;
        let name = name
            .into_string()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, &dict, default_max_burst_delay)?;
        let r_resp = SignalPathFromYaml::from_yaml_ref_with_prefix(
            common.module_scope(),
            &dict["r"]["resp"],
        )?;
        let full = match (
            SignalPathFromYaml::from_yaml_ref_with_prefix(common.module_scope(), &dict["r"]["id"]),
            SignalPathFromYaml::from_yaml_ref_with_prefix(common.module_scope(), &dict["ar"]["id"]),
            SignalPathFromYaml::from_yaml_ref_with_prefix(
                common.module_scope(),
                &dict["r"]["last"],
            ),
        ) {
            (Ok(r_id), Ok(ar_id), Ok(r_last)) => Some(AXIFullRd {
                r_id,
                ar_id,
                r_last,
            }),
            (Err(_), Err(_), Err(_)) => None,
            _ => Err("For AXI full all ar_id, r_id and r_last must be defined")?,
        };
        let mut dict = dict
            .into_hash()
            .ok_or("Channels description should not be empty")?;
        let ar = AXIBus::from_yaml(
            dict.remove(&yaml_rust2::Yaml::from_str(stringify!(ar)))
                .ok_or("AXI analyzer should have all channels defined")?,
            common.module_scope(),
        )?;
        let r = AXIBus::from_yaml(
            dict.remove(&yaml_rust2::Yaml::from_str(stringify!(r)))
                .ok_or("AXI analyzer should have all channels defined")?,
            common.module_scope(),
        )?;
        Ok(Self {
            common,
            ar,
            r,
            r_resp,
            full,
            result: None,
            window_length,
            x_rate,
            y_rate,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn calculate_lite(
        &self,
        usage: &mut MultiChannelBusUsage,
        mut ar: Peekable<ReadyValidTransactionIterator>,
        mut r: Peekable<ReadyValidTransactionIterator>,
        mut rst: RisingSignalIterator,
        r_resp: &Signal,
        last_time: &u32,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);
        while let Some(time) = ar.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            if let Some(&read_time) = r.peek()
                && next_rst > read_time
            {
                let next_transaction = ar.peek().unwrap_or(last_time);
                r.next();
                while let Some(&n) = r.peek()
                    && n < *next_transaction
                {
                    eprintln!("[WARN] Read without AR at {}", time_table[n as usize]);
                    r.next();
                }
                let resp = r_resp
                    .get_value_at(
                        &r_resp.get_offset(read_time).ok_or(format!(
                            "rresp is invalid at {}",
                            time_table[read_time as usize]
                        ))?,
                        0,
                    )
                    .to_bit_string()
                    .ok_or(format!(
                        "rresp is invalid at {}",
                        time_table[read_time as usize]
                    ))?;
                let [time, read_time, next_transaction] =
                    [time, read_time, *next_transaction].map(|i| time_table[i as usize]);
                usage.add_transaction(
                    time,
                    read_time,
                    read_time,
                    read_time,
                    &resp,
                    next_transaction,
                );
            } else {
                eprintln!(
                    "[WARN] unfinished transaction on {} at {}",
                    self.bus_name(),
                    time_table[time as usize]
                )
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn calculate_full(
        &self,
        usage: &mut MultiChannelBusUsage,
        mut ar: Peekable<ReadyValidTransactionIterator>,
        mut r: Peekable<ReadyValidTransactionIterator>,
        mut rst: RisingSignalIterator,
        r_resp: &Signal,
        ar_id: &Signal,
        r_id: &Signal,
        r_last: &Signal,
        last_time: &u32,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);
        let mut counting: HashMap<String, VecDeque<Transaction>> = HashMap::new();
        let mut unfinished = String::new();
        'transaction_loop: while let Some(time) = ar.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            let ar_id = get_id_value(ar_id, time)
                .ok_or(format!("arid is invalid at {}", time_table[time as usize]))?;
            let next_transaction = *ar.peek().unwrap_or(last_time);
            if let Some(transactions) = counting.get_mut(&ar_id) {
                transactions.push_back(Transaction::new(time, next_transaction));
            } else {
                counting.insert(
                    ar_id,
                    VecDeque::from([Transaction::new(time, next_transaction)]),
                );
            }
            while let Some(&read) = r.peek()
                && read < next_transaction
            {
                if read > next_rst {
                    unfinished.push_str(
                        &counting
                            .values()
                            .flat_map(|vec| {
                                vec.iter()
                                    .map(|t| time_table[t.start as usize].to_string())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    counting.clear();
                    continue 'transaction_loop;
                }
                r.next();
                let id = get_id_value(r_id, read)
                    .ok_or(format!("rid is invalid at {}", time_table[read as usize]))?;

                let Some(t_vec) = counting.get_mut(&id) else {
                    eprintln!(
                        "[WARN] R without AR on {} at {}",
                        self.bus_name(),
                        time_table[read as usize]
                    );
                    continue 'transaction_loop;
                };
                let Some(t) = t_vec.get_mut(0) else {
                    eprintln!(
                        "[WARN] R without AR on {} at {}",
                        self.bus_name(),
                        time_table[read as usize]
                    );
                    continue 'transaction_loop;
                };
                if t.first_data.is_none() {
                    t.first_data = Some(read)
                }
                if get_logic_value(r_last, read)
                    .ok_or(format!("rlast is invalid at {}", time_table[read as usize]))?
                    == ValueType::V1
                {
                    let resp = get_id_value(r_resp, read)
                        .ok_or(format!("rresp is invalid at {}", time_table[read as usize]))?;
                    let t = t_vec
                        .pop_front()
                        .expect("Already checked that transaction exists");
                    let [time, last_data, first_data, next_transaction] =
                        [t.start, read, t.first_data.expect("Should be set"), t.next]
                            .map(|i| time_table[i as usize]);
                    usage.add_transaction(
                        time,
                        last_data,
                        last_data,
                        first_data,
                        &resp,
                        next_transaction,
                    );
                }
            }
        }
        unfinished.push_str(
            &counting
                .values()
                .flat_map(|vec| {
                    vec.iter()
                        .map(|t| time_table[t.start as usize].to_string())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        if !unfinished.is_empty() {
            eprintln!(
                "[WARN] Unfinished transactions on {} at times: {}",
                self.bus_name(),
                unfinished
            );
        }
        Ok(())
    }
}

impl AnalyzerInternal for AXIRdAnalyzer {
    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.ar.signals());
        signals.append(&mut self.r.signals());
        signals.push(&self.r_resp);
        if let Some(full) = &self.full {
            signals.push(&full.ar_id);
            signals.push(&full.r_id);
            signals.push(&full.r_last);
        }

        signals
    }

    fn calculate(
        &mut self,
        loaded: Vec<&(wellen::SignalRef, Signal)>,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, _arready) = &loaded[2];
        let (_, arvalid) = &loaded[3];
        let (_, _rready) = &loaded[4];
        let (_, rvalid) = &loaded[5];
        let (_, r_resp) = &loaded[6];

        let mut reset = 0;

        let last_time = clk.time_indices().last().ok_or("clock has no values")?;
        let clock_period = *time_table.get(2).ok_or(
            "trace is too short (less than 3 time indices), cannot calculate clock period",
        )?;

        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            clock_period,
            self.x_rate,
            self.y_rate,
        );

        let intervals = if self.common.intervals().is_empty() {
            vec![[0, time_table[*last_time as usize]]]
        } else {
            self.common.intervals().clone()
        };
        for [start, end] in intervals.iter() {
            let start_idx = time_table
                .iter()
                .position(|time| time >= start)
                .ok_or("Invalid interval set")? as u32;
            let end_idx = time_table
                .iter()
                .rposition(|time| time <= end)
                .ok_or("Invalid interval set")? as u32;

            reset += count_reset(rst, self.common.rst_active_value(), start_idx, end_idx);
            let mut ar =
                ReadyValidTransactionIterator::new(clk, _arready, arvalid, end_idx).peekable();
            while ar.next_if(|t| *t < start_idx).is_some() {}
            let mut r =
                ReadyValidTransactionIterator::new(clk, _rready, rvalid, end_idx).peekable();
            while r.next_if(|t| *t < start_idx).is_some() {}
            let rst = RisingSignalIterator::new(rst);
            match self.full {
                Some(_) => {
                    let (_, ar_id) = &loaded[7];
                    let (_, r_id) = &loaded[8];
                    let (_, r_last) = &loaded[9];

                    self.calculate_full(
                        &mut usage, ar, r, rst, r_resp, ar_id, r_id, r_last, &end_idx, time_table,
                    )?;
                }
                None => {
                    self.calculate_lite(&mut usage, ar, r, rst, r_resp, &end_idx, time_table)?
                }
            }
            usage.add_time(end - start);
        }

        usage.end(reset, intervals);
        self.result = Some(BusUsage::MultiChannel(usage));
        Ok(())
    }

    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }
}

impl Analyzer for AXIRdAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }

    fn required_yaml_definitions(&self) -> Vec<&str> {
        Vec::from(AXI_RD_YAML)
    }
}

impl AXIWrAnalyzer {
    pub fn build_from_yaml(
        yaml: (yaml_rust2::Yaml, yaml_rust2::Yaml),
        default_max_burst_delay: CyclesNum,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Self, Box<dyn Error>> {
        let (name, dict) = yaml;
        let name = name
            .into_string()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, &dict, default_max_burst_delay)?;
        let b_resp = SignalPathFromYaml::from_yaml_ref_with_prefix(
            common.module_scope(),
            &dict["b"]["resp"],
        )?;
        let w_last = SignalPathFromYaml::from_yaml_ref_with_prefix(
            common.module_scope(),
            &dict["w"]["last"],
        );
        let aw_id =
            SignalPathFromYaml::from_yaml_ref_with_prefix(common.module_scope(), &dict["aw"]["id"]);
        let b_id =
            SignalPathFromYaml::from_yaml_ref_with_prefix(common.module_scope(), &dict["b"]["id"]);
        let full = match (aw_id, w_last, b_id) {
            (Ok(aw_id), Ok(w_last), Ok(b_id)) => Some(AXIFullWr {
                aw_id,
                w_last,
                b_id,
            }),
            (Err(_), Err(_), Err(_)) => None,
            (_, _, _) => Err("For AXI full all aw_id, w_last and b_id must be defined")?,
        };
        let mut dict = dict
            .into_hash()
            .ok_or("Channels description should not be empty")?;
        let aw = AXIBus::from_yaml(
            dict.remove(&yaml_rust2::Yaml::from_str(stringify!(aw)))
                .ok_or("AXI analyzer should have all channels defined")?,
            common.module_scope(),
        )?;
        let w = AXIBus::from_yaml(
            dict.remove(&yaml_rust2::Yaml::from_str(stringify!(w)))
                .ok_or("AXI analyzer should have all channels defined")?,
            common.module_scope(),
        )?;
        let b = AXIBus::from_yaml(
            dict.remove(&yaml_rust2::Yaml::from_str(stringify!(b)))
                .ok_or("AXI analyzer should have all channels defined")?,
            common.module_scope(),
        )?;
        Ok(Self {
            common,
            aw,
            w,
            b,
            b_resp,
            full,
            result: None,
            window_length,
            x_rate,
            y_rate,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn calculate_lite(
        &self,
        usage: &mut MultiChannelBusUsage,
        mut aw: Peekable<ReadyValidTransactionIterator>,
        mut w: Peekable<ReadyValidTransactionIterator>,
        mut b: Peekable<ReadyValidTransactionIterator>,
        b_resp: &Signal,
        mut rst: RisingSignalIterator,
        last_time: &u32,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);
        while let Some(time) = aw.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            if let Some(&data_time) = w.peek()
                && next_rst > data_time
                && let Some(&resp_time) = b.peek()
                && next_rst > resp_time
            {
                b.next();
                w.next().expect("Already checked");
                let next_transaction = aw.peek().unwrap_or(last_time);

                let resp = b_resp
                    .get_value_at(
                        &b_resp.get_offset(resp_time).ok_or(format!(
                            "bresp is invalid at {}",
                            time_table[resp_time as usize]
                        ))?,
                        0,
                    )
                    .to_bit_string()
                    .ok_or(format!(
                        "bresp is invalid at {}",
                        time_table[resp_time as usize]
                    ))?;
                let [time, resp_time, data_time, next_transaction] =
                    [time, resp_time, data_time, *next_transaction].map(|i| time_table[i as usize]);
                usage.add_transaction(
                    time,
                    resp_time,
                    data_time,
                    data_time,
                    &resp,
                    next_transaction,
                );
            } else {
                eprintln!(
                    "[WARN] unfinished transaction on {} at {}",
                    self.bus_name(),
                    time_table[time as usize]
                )
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn calculate_full(
        &self,
        usage: &mut MultiChannelBusUsage,
        mut aw: Peekable<ReadyValidTransactionIterator>,
        mut w: Peekable<ReadyValidTransactionIterator>,
        mut b: Peekable<ReadyValidTransactionIterator>,
        aw_id: &Signal,
        w_last: &Signal,
        b_id: &Signal,
        b_resp: &Signal,
        mut rst: RisingSignalIterator,
        last_time: &u32,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);
        let mut counting: HashMap<String, VecDeque<Transaction>> = HashMap::new();
        let mut unfinished = String::new();
        'transactions_loop: while let Some(time) = aw.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            let aw_id = get_id_value(aw_id, time)
                .ok_or(format!("awid is invalid at {}", time_table[time as usize]))?;
            let next_transaction = *aw.peek().unwrap_or(last_time);
            if let Some(transactions) = counting.get_mut(&aw_id) {
                transactions.push_back(Transaction::new(time, next_transaction));
            } else {
                counting.insert(
                    aw_id.clone(),
                    VecDeque::from([Transaction::new(time, next_transaction)]),
                );
            }

            let t = counting
                .get_mut(&aw_id)
                .expect("Should be valid because it's just been added")
                .back_mut()
                .expect("Should be valid because it's just been added");
            while let Some(&write) = w.peek() {
                if write > next_rst {
                    unfinished.push_str(
                        &counting
                            .values()
                            .flat_map(|vec| {
                                vec.iter()
                                    .map(|t| time_table[t.start as usize].to_string())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    counting.clear();
                    continue 'transactions_loop;
                }
                w.next();
                if t.first_data.is_none() {
                    t.first_data = Some(write);
                }
                if get_logic_value(w_last, write).ok_or(format!(
                    "wlast is invalid at {}",
                    time_table[write as usize]
                ))? == ValueType::V1
                {
                    t.last_data = Some(write);
                    break;
                }
            }

            while let Some(&resp_time) = b.peek()
                && resp_time < next_transaction
            {
                if resp_time > next_rst {
                    unfinished.push_str(
                        &counting
                            .values()
                            .flat_map(|vec| {
                                vec.iter()
                                    .map(|t| time_table[t.start as usize].to_string())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    counting.clear();
                    continue 'transactions_loop;
                }
                b.next();
                let b_id = get_id_value(b_id, resp_time).ok_or(format!(
                    "bid is invalid at {}",
                    time_table[resp_time as usize]
                ))?;
                let Some(t_vec) = counting.get_mut(&b_id) else {
                    eprintln!(
                        "[WARN] Transaction response without command at {}",
                        time_table[resp_time as usize]
                    );
                    continue;
                };
                let Some(t) = t_vec.pop_front() else {
                    eprintln!(
                        "[WARN] Transaction response without command at {}",
                        time_table[resp_time as usize]
                    );
                    continue;
                };

                let resp = get_id_value(b_resp, resp_time).ok_or(format!(
                    "bresp is invalid at {}",
                    time_table[resp_time as usize]
                ))?;

                let [time, resp_time, last_data, first_data, next_transaction] = [
                    t.start,
                    resp_time,
                    t.last_data.ok_or(format!("wlast was not asserted before response for transaction at {}-{}", t.start, t.next))?,
                    t.first_data.ok_or(format!("no data was transfered nor wlast was not asserted before response for transaction at {}-{}", t.start, t.next))?,
                    t.next,
                ]
                .map(|i| time_table[i as usize]);
                usage.add_transaction(
                    time,
                    resp_time,
                    last_data,
                    first_data,
                    &resp,
                    next_transaction,
                );
            }
        }
        unfinished.push_str(
            &counting
                .values()
                .flat_map(|vec| {
                    vec.iter()
                        .map(|t| time_table[t.start as usize].to_string())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        if !unfinished.is_empty() {
            eprintln!(
                "[WARN] Unfinished transactions on {} at times: {}",
                self.bus_name(),
                unfinished
            );
        }
        Ok(())
    }
}

impl AnalyzerInternal for AXIWrAnalyzer {
    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.aw.signals());
        signals.append(&mut self.w.signals());
        signals.append(&mut self.b.signals());
        signals.push(&self.b_resp);
        if let Some(full) = &self.full {
            signals.push(&full.aw_id);
            signals.push(&full.w_last);
            signals.push(&full.b_id);
        }

        signals
    }

    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }

    fn calculate(
        &mut self,
        loaded: Vec<&(wellen::SignalRef, Signal)>,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, awready) = &loaded[2];
        let (_, awvalid) = &loaded[3];
        let (_, wready) = &loaded[4];
        let (_, wvalid) = &loaded[5];
        let (_, bready) = &loaded[6];
        let (_, bvalid) = &loaded[7];
        let (_, b_resp) = &loaded[8];

        let mut reset = 0;
        let last_time = clk
            .time_indices()
            .last()
            .ok_or("Clock should have values")?;
        let clock_period = *time_table.get(2).ok_or(
            "trace is too short (less than 3 time indices), cannot calculate clock period",
        )?;
        let intervals = if self.common.intervals().is_empty() {
            vec![[0, time_table[*last_time as usize]]]
        } else {
            self.common.intervals().clone()
        };

        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            clock_period,
            self.x_rate,
            self.y_rate,
        );

        for [start, end] in intervals.iter() {
            let start_idx = time_table
                .iter()
                .position(|time| time >= start)
                .ok_or("Invalid interval set")? as u32;
            let end_idx = time_table
                .iter()
                .rposition(|time| time <= end)
                .ok_or("Invalid interval set")? as u32;

            reset += count_reset(rst, self.common.rst_active_value(), start_idx, end_idx);

            let mut aw =
                ReadyValidTransactionIterator::new(clk, awready, awvalid, end_idx).peekable();
            while aw.next_if(|t| *t < start_idx).is_some() {}
            let mut w = ReadyValidTransactionIterator::new(clk, wready, wvalid, end_idx).peekable();
            while w.next_if(|t| *t < start_idx).is_some() {}
            let mut b = ReadyValidTransactionIterator::new(clk, bready, bvalid, end_idx).peekable();
            while b.next_if(|t| *t < start_idx).is_some() {}
            let rst = RisingSignalIterator::new(rst);

            match self.full {
                Some(_) => {
                    let (_, aw_id) = &loaded[9];
                    let (_, w_last) = &loaded[10];
                    let (_, b_id) = &loaded[11];
                    self.calculate_full(
                        &mut usage, aw, w, b, aw_id, w_last, b_id, b_resp, rst, &end_idx,
                        time_table,
                    )?;
                }
                None => {
                    self.calculate_lite(&mut usage, aw, w, b, b_resp, rst, &end_idx, time_table)?
                }
            }
            usage.add_time(end - start);
        }

        usage.end(reset, intervals);
        self.result = Some(BusUsage::MultiChannel(usage));
        Ok(())
    }
}

impl Analyzer for AXIWrAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }

    fn required_yaml_definitions(&self) -> Vec<&str> {
        Vec::from(AXI_WR_YAML)
    }
}

struct RisingSignalIterator<'a> {
    signal: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    peeked: Option<TimeTableIdx>,
}

impl<'a> RisingSignalIterator<'a> {
    fn new(signal: &'a Signal) -> Self {
        let signal: Box<dyn Iterator<Item = _>> = Box::new(signal.iter_changes());
        let signal = signal.peekable();
        Self {
            signal,
            peeked: None,
        }
    }

    fn find_non_consuming<P>(&mut self, mut predicate: P) -> Option<TimeTableIdx>
    where
        P: FnMut(&TimeTableIdx) -> bool,
    {
        loop {
            if let Some(t) = self.next() {
                if predicate(&t) {
                    self.peeked = Some(t);
                    break Some(t);
                }
            } else {
                break None;
            }
        }
    }
}

impl<'a> Iterator for RisingSignalIterator<'a> {
    type Item = TimeTableIdx;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(t) = self.peeked {
            self.peeked = None;
            Some(t)
        } else {
            loop {
                match self.signal.next() {
                    Some((_, value)) => {
                        if matches!(get_value(value), Some(ValueType::V0))
                            && let Some((time, next_value)) = self.signal.next()
                            && matches!(get_value(next_value), Some(ValueType::V1))
                        {
                            return Some(time);
                        }
                    }
                    None => return None,
                }
            }
        }
    }
}

pub struct ReadyValidTransactionIterator<'a> {
    current_time: TimeTableIdx,
    clk: RisingSignalIterator<'a>,
    ready: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    valid: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    time_end: TimeTableIdx,
}

impl<'a> ReadyValidTransactionIterator<'a> {
    pub fn new(
        clk: &'a Signal,
        ready: &'a Signal,
        valid: &'a Signal,
        time_end: TimeTableIdx,
    ) -> Self {
        let mut current_time;
        let clk = RisingSignalIterator::new(clk);
        let ready: Box<dyn Iterator<Item = (u32, SignalValue)>> = Box::new(ready.iter_changes());
        let valid: Box<dyn Iterator<Item = (u32, SignalValue)>> = Box::new(valid.iter_changes());
        let mut ready = ready.peekable();
        let mut valid = valid.peekable();
        let first_ready = ready.find(|(_, value)| matches!(get_value(*value), Some(ValueType::V1)));
        match first_ready {
            Some((time, _)) => current_time = time,
            None => current_time = time_end,
        };
        let first_valid = valid.find(|(_, value)| matches!(get_value(*value), Some(ValueType::V1)));
        match first_valid {
            Some((time, _)) => current_time = current_time.max(time),
            None => current_time = time_end,
        }

        Self {
            current_time,
            clk,
            ready,
            valid,
            time_end,
        }
    }
}

impl<'a> Iterator for ReadyValidTransactionIterator<'a> {
    type Item = TimeTableIdx;

    fn next(&mut self) -> Option<Self::Item> {
        // Find next clock rising edge
        self.current_time = loop {
            if let Some(time) = self.clk.next() {
                if time > self.current_time {
                    break time;
                }
            } else {
                return None;
            }
        };
        // Check if either of ready or valid changed to value 0
        // if so set current_time to that time and perform the check again
        while let Some(smaller) = match (self.ready.peek(), self.valid.peek()) {
            (None, None) => None,
            (None, Some(_)) => Some(&mut self.valid),
            (Some(_), None) => Some(&mut self.ready),
            (Some(ready), Some(valid)) => Some(if ready.0 > valid.0 {
                &mut self.valid
            } else {
                &mut self.ready
            }),
        } {
            if self.current_time > self.time_end {
                return None;
            }
            let &(smaller_next, _) = smaller.peek().expect("Already checked");
            if self.current_time > smaller_next {
                while smaller
                    .next_if(|(_, v)| match get_value(*v) {
                        Some(v) => !matches!(v, ValueType::V1),
                        None => true,
                    })
                    .is_some()
                {}

                match smaller.next() {
                    #[allow(clippy::unwrap_used)]
                    Some((time, v)) => {
                        debug_assert!(
                            matches!(get_value(v).unwrap(), ValueType::V1),
                            "Next change should be to value 1"
                        );
                        if time >= self.current_time {
                            self.current_time = self
                                .clk
                                .find_non_consuming(|&t| t > time)
                                .unwrap_or(self.time_end);
                        }
                    }
                    None => return None,
                }
            } else {
                return Some(self.current_time);
            }
        }
        Some(self.current_time)
    }
}
