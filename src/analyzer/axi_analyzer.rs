use std::{error::Error, iter::Peekable};

use wellen::{Signal, SignalValue, TimeTable, TimeTableIdx};

use crate::{
    analyzer::private::AnalyzerInternal,
    bus::{
        BusCommon, BusDescription, CyclesNum, SignalPath, ValueType, axi::AXIBus, get_value,
        is_value_of_type,
    },
    bus_usage::{BusUsage, MultiChannelBusUsage},
};

use super::Analyzer;

pub struct AXIRdAnalyzer {
    common: BusCommon,
    ar: AXIBus,
    r: AXIBus,
    r_resp: SignalPath,
    result: Option<BusUsage>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

pub struct AXIWrAnalyzer {
    common: BusCommon,
    aw: AXIBus,
    w: AXIBus,
    /// w_last is optional, if it's None we assume all transactions have len = 1
    w_last: Option<SignalPath>,
    b: AXIBus,
    b_resp: SignalPath,
    result: Option<BusUsage>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

// Count how many clock cycles was reset active
fn count_reset(rst: &Signal, active_value: ValueType) -> u32 {
    let mut last = 0;
    let mut reset = 0;
    for (time, value) in rst.iter_changes() {
        if is_value_of_type(value, active_value) {
            last = time;
        } else {
            reset += time - last;
        }
    }
    reset / 2
}

macro_rules! build_from_yaml {
    ( $( $bus_name:tt $bus_type:ident ),* ; $([$signal_name:ident $($signal_init:tt)*]),* ) => {
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
            $(
                let $signal_name = SignalPath::from_yaml_ref_with_prefix(
                    common.module_scope(),
                    &dict$($signal_init)*)?;
            )*
            let mut dict =  dict.into_hash().ok_or("Channels description should not be empty")?;
            $(
                let $bus_name = $bus_type::from_yaml(
                    dict.remove(&yaml_rust2::Yaml::from_str(stringify!($bus_name))).ok_or("AXI analyzer should have all channels defined")?,
                    common.module_scope(),
                )?;
            )*
            Ok(Self {
                common,
                $($bus_name,)*
                $($signal_name,)*
                result: None,
                window_length,
                x_rate,
                y_rate,
            })
        }
    };
}

impl AXIRdAnalyzer {
    build_from_yaml!(ar AXIBus, r AXIBus; [ r_resp ["r"]["rresp"] ]);
}

impl AnalyzerInternal for AXIRdAnalyzer {
    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.ar.signals());
        signals.append(&mut self.r.signals());
        signals.push(&self.r_resp);

        signals
    }

    fn calculate(&mut self, loaded: Vec<(wellen::SignalRef, Signal)>, time_table: &TimeTable) {
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, _arready) = &loaded[2];
        let (_, arvalid) = &loaded[3];
        let (_, _rready) = &loaded[4];
        let (_, rvalid) = &loaded[5];
        let (_, r_resp) = &loaded[6];

        let reset = count_reset(rst, self.common.rst_active_value());

        let last_time = clk.time_indices().last().expect("Clock should have values");
        let clock_period = time_table[2];

        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            clock_period,
            self.x_rate,
            self.y_rate,
            time_table[*last_time as usize],
        );

        let mut ar =
            ReadyValidTransactionIterator::new(clk, _arready, arvalid, *last_time).peekable();
        let mut r = ReadyValidTransactionIterator::new(clk, _rready, rvalid, *last_time).peekable();
        let mut rst = RisingSignalIterator::new(rst);
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);

        while let Some(time) = ar.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            if let Some(&first_data) = r.peek()
                && next_rst > first_data
            {
                let next_transaction = ar.peek().unwrap_or(last_time);
                let mut last_data = r.next().expect("Already checked");
                while let Some(&n) = r.peek()
                    && n < *next_transaction
                {
                    last_data = n;
                    r.next();
                }
                let resp_time = last_data;
                let resp = r_resp
                    .get_value_at(
                        &r_resp
                            .get_offset(resp_time)
                            .expect("There should be a response available at response time"),
                        0,
                    )
                    .to_bit_string()
                    .expect("Function never returns None");
                let [time, resp_time, last_data, first_data, next_transaction] =
                    [time, resp_time, last_data, first_data, *next_transaction]
                        .map(|i| time_table[i as usize]);
                usage.add_transaction(
                    time,
                    resp_time,
                    last_data,
                    first_data,
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
        usage.end(reset);
        self.result = Some(BusUsage::MultiChannel(usage));
    }

    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }
}

impl Analyzer for AXIRdAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
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
        let b_resp =
            SignalPath::from_yaml_ref_with_prefix(common.module_scope(), &dict["b"]["bresp"])?;
        let w_last =
            SignalPath::from_yaml_ref_with_prefix(common.module_scope(), &dict["w"]["wlast"]).ok();
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
            w_last,
            result: None,
            window_length,
            x_rate,
            y_rate,
        })
    }
}

impl AnalyzerInternal for AXIWrAnalyzer {
    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.aw.signals());
        signals.append(&mut self.w.signals());
        signals.append(&mut self.b.signals());
        signals.push(&self.b_resp);
        if let Some(w_last) = &self.w_last {
            signals.push(w_last);
        }

        signals
    }

    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }

    fn calculate(&mut self, loaded: Vec<(wellen::SignalRef, Signal)>, time_table: &TimeTable) {
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, awready) = &loaded[2];
        let (_, awvalid) = &loaded[3];
        let (_, wready) = &loaded[4];
        let (_, wvalid) = &loaded[5];
        let (_, bready) = &loaded[6];
        let (_, bvalid) = &loaded[7];
        let (_, b_resp) = &loaded[8];

        let reset = count_reset(rst, self.common.rst_active_value());
        let last_time = clk.time_indices().last().expect("Clock should have values");
        let clock_period = time_table[2];

        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            clock_period,
            self.x_rate,
            self.y_rate,
            time_table[*last_time as usize],
        );

        let mut aw =
            ReadyValidTransactionIterator::new(clk, awready, awvalid, *last_time).peekable();
        let mut w = ReadyValidTransactionIterator::new(clk, wready, wvalid, *last_time).peekable();
        let mut b = ReadyValidTransactionIterator::new(clk, bready, bvalid, *last_time).peekable();
        let mut rst = RisingSignalIterator::new(rst);
        let mut next_rst = rst.next().unwrap_or(*last_time + 1);

        'transactions_loop: while let Some(time) = aw.next() {
            while next_rst < time {
                next_rst = rst.next().unwrap_or(*last_time + 1);
            }
            if let Some(&first_data) = w.peek()
                && next_rst > first_data
                && let Some(&resp_time) = b.peek()
                && next_rst > resp_time
            {
                b.next();
                let next_transaction = aw.peek().unwrap_or(last_time);
                let last_data = if self.w_last.is_some() {
                    let (_, w_last) = &loaded[9];
                    loop {
                        let Some(last_data) = w.next() else {
                            eprintln!(
                                "[WARN] Transaction without w_last assertion on {} at {}",
                                self.bus_name(),
                                time_table[time as usize]
                            );
                            continue 'transactions_loop;
                        };
                        if get_value(w_last.get_value_at(
                            &w_last.get_offset(last_data).expect("Should be valid"),
                            0,
                        ))
                        .expect("Should be valid")
                            == ValueType::V1
                        {
                            break last_data;
                        }
                    }
                } else {
                    w.next().expect("Already checked")
                };

                let resp = b_resp
                    .get_value_at(
                        &b_resp
                            .get_offset(resp_time)
                            .expect("There should be a response available at response time"),
                        0,
                    )
                    .to_bit_string()
                    .expect("Function never returns None");
                let [time, resp_time, last_data, first_data, next_transaction] =
                    [time, resp_time, last_data, first_data, *next_transaction]
                        .map(|i| time_table[i as usize]);
                usage.add_transaction(
                    time,
                    resp_time,
                    last_data,
                    first_data,
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

        usage.end(reset);
        self.result = Some(BusUsage::MultiChannel(usage));
    }
}

impl Analyzer for AXIWrAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }
}

struct RisingSignalIterator<'a> {
    signal: Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>,
}

impl<'a> RisingSignalIterator<'a> {
    fn new(signal: &'a Signal) -> Self {
        let signal = Box::new(signal.iter_changes());
        Self { signal }
    }
}

impl<'a> Iterator for RisingSignalIterator<'a> {
    type Item = TimeTableIdx;

    fn next(&mut self) -> Option<Self::Item> {
        match self.signal.next() {
            Some((time, value)) => {
                if matches!(
                    get_value(value).expect("Value should be valid"),
                    ValueType::V1
                ) {
                    Some(time)
                } else {
                    self.signal.next().map(|(time, _)| time)
                }
            }
            None => None,
        }
    }
}

struct ReadyValidTransactionIterator<'a> {
    current_time: TimeTableIdx,
    clk: Peekable<RisingSignalIterator<'a>>,
    ready: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    valid: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    time_end: TimeTableIdx,
}

impl<'a> ReadyValidTransactionIterator<'a> {
    fn new(clk: &'a Signal, ready: &'a Signal, valid: &'a Signal, time_end: TimeTableIdx) -> Self {
        let mut current_time;
        let clk = RisingSignalIterator::new(clk).peekable();
        let ready: Box<dyn Iterator<Item = (u32, SignalValue)>> = Box::new(ready.iter_changes());
        let valid: Box<dyn Iterator<Item = (u32, SignalValue)>> = Box::new(valid.iter_changes());
        let mut ready = ready.peekable();
        let mut valid = valid.peekable();
        let first_ready = ready.find(|(_, value)| {
            matches!(
                get_value(*value).expect("Signal value should be valid"),
                ValueType::V1
            )
        });
        match first_ready {
            Some((time, _)) => current_time = time.saturating_sub(2),
            None => current_time = time_end,
        };
        let first_valid = valid.find(|(_, value)| {
            matches!(
                get_value(*value).expect("Signal value should be valid"),
                ValueType::V1
            )
        });
        match first_valid {
            Some((time, _)) => current_time = current_time.max(time.saturating_sub(2)),
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
        if self.current_time > self.time_end {
            return None;
        }
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
            let &(smaller_next, _) = smaller.peek().expect("Already checked");
            if self.current_time >= smaller_next {
                let (_time, v) = smaller.next().expect("Already checked in first if");
                debug_assert!(
                    matches!(get_value(v).unwrap(), ValueType::V0),
                    "Next change should be to value 0"
                );
                match smaller.next() {
                    Some((time, v)) => {
                        debug_assert!(
                            matches!(get_value(v).unwrap(), ValueType::V1),
                            "Next change should be to value 1"
                        );
                        self.current_time = time.max(self.current_time);
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
