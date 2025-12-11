use crate::CycleType;
use crate::{CyclesNum, SignalPath};
use std::collections::HashMap;

#[derive(bincode::Encode, bincode::Decode)]
pub struct BusData {
    pub usage: BusUsage,
    pub signals: Vec<SignalPath>,
}

impl BusData {
    pub fn new(usage: BusUsage, signals: Vec<SignalPath>) -> Self {
        Self { usage, signals }
    }
}

/// Enum that contains all bus usage types.
#[derive(PartialEq, Debug, Clone, bincode::Encode, bincode::Decode)]
pub enum BusUsage {
    SingleChannel(SingleChannelBusUsage),
    MultiChannel(MultiChannelBusUsage),
}

impl BusUsage {
    pub fn get_name(&self) -> &str {
        match self {
            BusUsage::SingleChannel(single_channel_bus_usage) => &single_channel_bus_usage.bus_name,
            BusUsage::MultiChannel(multi_channel_bus_usage) => &multi_channel_bus_usage.bus_name,
        }
    }
    pub fn get_statistics<'a>(&'a self, skipped_stats: &[String]) -> Vec<Statistic<'a>> {
        match self {
            BusUsage::SingleChannel(single_channel_bus_usage) => {
                single_channel_bus_usage.get_statistics()
            }
            BusUsage::MultiChannel(multi_channel_bus_usage) => {
                multi_channel_bus_usage.get_statistics(skipped_stats)
            }
        }
    }
}

/// Enum that contains all statistic types.
pub enum Statistic<'a> {
    Percentage(PercentageStatistic),
    Bucket(BucketsStatistic<'a>),
    Timeline(TimelineStatistic),
}

impl<'a> Statistic<'a> {
    pub fn name(&self) -> &'static str {
        match self {
            Statistic::Percentage(percentage_statistic) => percentage_statistic.name,
            Statistic::Bucket(buckets_statistic) => buckets_statistic.name,
            Statistic::Timeline(timeline_statistic) => timeline_statistic.name,
        }
    }
}

/// Statistic that compares given values based on their proportions.
pub struct PercentageStatistic {
    pub name: &'static str,
    pub data_labels: Vec<(f32, &'static str)>,
    pub description: &'static str,
}

impl PercentageStatistic {
    pub fn new(
        name: &'static str,
        data_labels: Vec<(f32, &'static str)>,
        description: &'static str,
    ) -> Self {
        PercentageStatistic {
            name,
            data_labels,
            description,
        }
    }

    pub fn display(&self) -> String {
        self.data_labels
            .iter()
            .map(|(v, l)| format!("{l}: {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Statistic that describes values continously changing in time.
pub struct TimelineStatistic {
    pub name: &'static str,
    pub values: Vec<[f64; 2]>,
    pub vertical_lines: Vec<f64>,
    pub display: String,
    pub description: &'static str,
}

impl TimelineStatistic {
    pub fn get_data(&self) -> &Vec<[f64; 2]> {
        &self.values
    }
}

/// Stores in what state is the bus currently
#[doc(hidden)]
#[derive(PartialEq, Debug, Clone, bincode::Encode, bincode::Decode)]
pub enum CurrentlyCalculating {
    None,
    Burst,
    Delay,
    /// Delay during burst
    Pause(CyclesNum),
}

/// Contains statistics for a single channel bus.
#[derive(PartialEq, Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct SingleChannelBusUsage {
    pub bus_name: String,
    busy: CyclesNum,
    backpressure: CyclesNum,
    no_data: CyclesNum,
    no_transaction: CyclesNum,
    free: CyclesNum,
    reset: CyclesNum,

    transaction_delays: Vec<Period>,
    burst_lengths: Vec<Period>,
    current: CurrentlyCalculating,

    max_burst_delay: CyclesNum,
    clk_period: RealTime,
}

impl SingleChannelBusUsage {
    pub fn get_statistics<'a>(&'a self) -> Vec<Statistic<'a>> {
        Vec::from([
            Statistic::Percentage(self.get_cycles()),
            Statistic::Bucket(BucketsStatistic {
                name: "Transaction delays",
                data: &self.transaction_delays,
                clk_to_time: self.clk_period,
                color: "Red",
                description: "Delays between transaction in clock cycles",
            }),
            Statistic::Bucket(BucketsStatistic {
                name: "Burst lengths",
                data: &self.burst_lengths,
                clk_to_time: self.clk_period,
                color: "Blue",
                description: "Burst lengths in clock cycles",
            }),
        ])
    }
    fn get_cycles(&self) -> PercentageStatistic {
        PercentageStatistic {
            data_labels: vec![
                (self.busy as f32, "Busy"),
                (self.backpressure as f32, "Backpressure"),
                (self.no_data as f32, "No data"),
                (self.no_transaction as f32, "No transaction"),
                (self.free as f32, "Free"),
                (self.reset as f32, "Reset"),
            ],
            name: "Cycles",
            description: "How many clock cycles was bus in each state",
        }
    }
    /// Creates SingleChannelBusUsage with all statistics initialized to 0.
    /// To fill it with data use add_cycle() method for every cycle in the simulation. Later call end() to finish calculations.
    pub fn new(
        name: &str,
        max_burst_delay: CyclesNum,
        clk_to_time: u64,
    ) -> SingleChannelBusUsage {
        SingleChannelBusUsage {
            bus_name: name.to_owned(),
            busy: 0,
            backpressure: 0,
            no_data: 0,
            no_transaction: 0,
            free: 0,
            reset: 0,
            transaction_delays: vec![],
            burst_lengths: vec![],
            current: CurrentlyCalculating::None,
            max_burst_delay,
            clk_period: clk_to_time,
        }
    }

    /// Updates statistics by adding a cycle of given type
    pub fn add_cycle(&mut self, t: CycleType) {
        if let CycleType::Busy = t {
            self.add_busy_cycle();
        } else {
            self.add_wasted_cycle(t);
        }
    }

    fn add_busy_cycle(&mut self) {
        match self.current {
            CurrentlyCalculating::None => {
                self.burst_lengths
                    .push(Period::with_duration(0, 1, self.clk_period));
                self.current = CurrentlyCalculating::Burst;
            }
            CurrentlyCalculating::Burst => {
                self.burst_lengths
                    .last_mut()
                    .expect("Should have at least one")
                    .add_cycle(self.clk_period);
            }
            CurrentlyCalculating::Delay => {
                let delay = self
                    .transaction_delays
                    .last()
                    .expect("Should have at least one");
                self.burst_lengths.push(Period::with_duration(
                    delay.end() + self.clk_period,
                    1,
                    self.clk_period,
                ));
                self.current = CurrentlyCalculating::Burst;
            }
            CurrentlyCalculating::Pause(duration) => {
                self.burst_lengths
                    .last_mut()
                    .expect("Should have at least one")
                    .add_n_cycles(duration + 1, self.clk_period);
                self.current = CurrentlyCalculating::Burst;
            }
        }
        self.busy += 1;
    }

    fn add_wasted_cycle(&mut self, t: CycleType) {
        match t {
            CycleType::Free => self.free += 1,
            CycleType::NoTransaction => self.no_transaction += 1,
            CycleType::Backpressure => self.backpressure += 1,
            CycleType::NoData => self.no_data += 1,
            CycleType::Reset => self.reset += 1,
            CycleType::Busy => unreachable!(),
            CycleType::Unknown => self.no_transaction += 1,
        }
        match self.current {
            CurrentlyCalculating::None => {
                self.transaction_delays
                    .push(Period::with_duration(0, 1, self.clk_period));
                self.current = CurrentlyCalculating::Delay
            }
            CurrentlyCalculating::Burst => {
                if self.max_burst_delay == 0 {
                    let transaction_end = self
                        .burst_lengths
                        .last()
                        .expect("Should have at least one")
                        .end();
                    self.transaction_delays.push(Period::with_duration(
                        transaction_end + self.clk_period,
                        1,
                        self.clk_period,
                    ));
                    self.current = CurrentlyCalculating::Delay;
                } else {
                    self.current = CurrentlyCalculating::Pause(1);
                }
            }
            CurrentlyCalculating::Delay => {
                self.transaction_delays
                    .last_mut()
                    .expect("Should have at least one")
                    .add_cycle(self.clk_period);
            }
            CurrentlyCalculating::Pause(duration) => {
                if duration + 1 > self.max_burst_delay {
                    let transaction_end = self
                        .burst_lengths
                        .last()
                        .expect("Should have at least one")
                        .end();
                    self.transaction_delays.push(Period::with_duration(
                        transaction_end + self.clk_period,
                        duration + 1,
                        self.clk_period,
                    ));
                    self.current = CurrentlyCalculating::Delay;
                } else {
                    self.current = CurrentlyCalculating::Pause(duration + 1);
                }
            }
        }
    }

    /// Creates SingleChannelBusUsage with given values - for tests purposes
    #[allow(clippy::too_many_arguments)]
    pub fn literal(
        bus_name: &str,
        busy: CyclesNum,
        backpressure: CyclesNum,
        no_data: CyclesNum,
        no_transaction: CyclesNum,
        free: CyclesNum,
        reset: CyclesNum,
        transaction_delays: Vec<Period>,
        burst_lengths: Vec<Period>,
        max_burst_delay: CyclesNum,
        current: CurrentlyCalculating,
        clk_to_time: u64,
    ) -> SingleChannelBusUsage {
        SingleChannelBusUsage {
            bus_name: bus_name.to_owned(),
            busy,
            backpressure,
            no_data,
            no_transaction,
            free,
            reset,
            transaction_delays,
            burst_lengths,
            max_burst_delay,
            current,
            clk_period: clk_to_time,
        }
    }
}

/// Statistic that groups the periods by their duration and counts how many of them are in each bucket.
#[derive(PartialEq, Debug, Clone)]
pub struct BucketsStatistic<'a> {
    pub name: &'static str,
    pub data: &'a Vec<Period>,
    // Clock period.
    pub clk_to_time: u64,
    pub color: &'static str,
    pub description: &'static str,
}

impl<'a> BucketsStatistic<'a> {
    pub fn new(
        name: &'static str,
        data: &'a Vec<Period>,
        clk_to_time: u64,
        color: &'static str,
        description: &'static str,
    ) -> BucketsStatistic<'a> {
        BucketsStatistic {
            name,
            data,
            clk_to_time,
            color,
            description,
        }
    }
    /// Returns counts of periods that are of each size
    pub fn get_data(&self) -> HashMap<CyclesNum, usize> {
        let mut buckets = HashMap::new();
        for v in self.data.iter() {
            let v = v.duration;
            match buckets.get_mut(&v) {
                Some(num) => {
                    *num += 1;
                }
                None => {
                    buckets.insert(v, 1);
                }
            }
        }
        buckets
    }
    /// Returns values of the buckets that are of logarythmic scale size
    pub fn get_buckets(&self) -> HashMap<CyclesNum, usize> {
        let mut buckets = HashMap::new();
        for v in self.data.iter() {
            let bucket = match v.duration {
                0 => 0,
                v if v > 0 => (v.ilog2() + 1) as i32,
                v => -(v.abs().ilog2() as i32 + 1),
            };
            match buckets.get_mut(&bucket) {
                Some(num) => *num += 1,
                None => {
                    buckets.insert(bucket, 1);
                }
            }
        }
        buckets
    }
    fn bucket_num(cycle_num: CyclesNum) -> i32 {
        match cycle_num {
            0 => 0,
            v if v > 0 => (v.ilog2() + 1) as i32,
            v => -(v.abs().ilog2() as i32 + 1),
        }
    }
    // Returns periods that have specified duration
    pub fn get_data_of_value(&self, value: CyclesNum) -> Vec<Period> {
        self.data
            .iter()
            .filter(|d| d.duration == value)
            .copied()
            .collect()
    }
    // Returns periods that belong to bucket nr [bucket_num]
    pub fn get_data_for_bucket(&self, bucket_num: i32) -> Vec<Period> {
        self.data
            .iter()
            .filter(|d| {
                let bucket = BucketsStatistic::bucket_num(d.duration);
                bucket == bucket_num
            })
            .copied()
            .collect()
    }

    pub fn buckets_num(&self) -> u32 {
        self.get_buckets().len() as u32
    }
    pub fn display(&self) -> String {
        let name = self.name;
        if let Some(min) = self.data.iter().map(|d| d.duration).min()
            && let Some(max) = self.data.iter().map(|d| d.duration).max()
        {
            format!("{name}: {min}-{max} clock cycles")
        } else {
            format!("{name}: no data")
        }
    }
}

/// Waveform time.
pub type RealTime = u64;
type SignedRealTime = i64;

/// Contains waveform times of start and end of some period and its duration in clock cycles.
#[derive(PartialEq, Debug, Clone, Copy, bincode::Encode, bincode::Decode)]
pub struct Period {
    start: RealTime,
    end: RealTime,
    duration: CyclesNum,
}

impl Period {
    fn new(start: RealTime, end: RealTime, clk_period: RealTime) -> Self {
        let duration = ((end as SignedRealTime - start as SignedRealTime)
            / clk_period as SignedRealTime) as CyclesNum;
        Self {
            start,
            end,
            duration,
        }
    }
    fn with_duration(start: RealTime, duration: CyclesNum, clk_period: RealTime) -> Self {
        let end = start + (duration - 1) as u64 * clk_period;
        Self {
            start,
            end,
            duration,
        }
    }
    // Method for writing tests, you most likely want to use [Period::new] or [Period::with_duration]
    pub fn literal(start: RealTime, end: RealTime, duration: CyclesNum) -> Self {
        Self {
            start,
            end,
            duration,
        }
    }
    #[inline]
    pub fn add_cycle(&mut self, clk_period: RealTime) {
        self.add_n_cycles(1, clk_period);
    }
    pub fn add_n_cycles(&mut self, n: CyclesNum, clk_period: RealTime) {
        let added_time = n as u64 * clk_period;
        self.end += added_time;
        self.duration += n;
    }
    pub fn start(&self) -> RealTime {
        self.start
    }
    pub fn end(&self) -> RealTime {
        self.end
    }
    pub fn duration(&self) -> CyclesNum {
        self.duration
    }
}

/// Contains statistics for a multichannel bus.
#[derive(PartialEq, Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct MultiChannelBusUsage {
    pub bus_name: String,
    cmd_to_completion: Vec<Period>,
    cmd_to_first_data: Vec<Period>,
    last_data_to_completion: Vec<Period>,
    transaction_delays: Vec<Period>,
    error_rate: f32,
    errors: Vec<RealTime>,
    // Temporary value - number of correct transactions
    correct_num: u32,
    averaged_bandwidth: f32,
    bandwidth_windows: Vec<[f64; 2]>,
    window_length: u32,
    clock_period: RealTime,
    bandwidth_above_x_rate: f32,
    bandwidth_below_y_rate: f32,
    time: RealTime,
    /// We have a statistic that calculates % of time that the bandwidth was ABOVE this value
    x_rate: f64,
    /// We have a statistic that calculates % of time that the bandwidth was BELOW this value
    y_rate: f64,
    intervals: Vec<[u64; 2]>,
}

impl MultiChannelBusUsage {
    /// Creates empty MultiChannelBusUsage with all statistics initialized to zero. Should be filled with add_transaction()
    pub fn new(
        bus_name: &str,
        window_length: u32,
        clock_period: RealTime,
        x_rate: f32,
        y_rate: f32,
    ) -> Self {
        MultiChannelBusUsage {
            bus_name: bus_name.to_owned(),
            cmd_to_completion: vec![],
            cmd_to_first_data: vec![],
            last_data_to_completion: vec![],
            transaction_delays: vec![],
            error_rate: 0.0,
            errors: vec![],
            correct_num: 0,
            averaged_bandwidth: 0.0,
            bandwidth_windows: vec![],
            window_length,
            clock_period,
            bandwidth_above_x_rate: 0.0,
            bandwidth_below_y_rate: 0.0,
            time: 0,
            x_rate: x_rate as f64,
            y_rate: y_rate as f64,
            intervals: vec![],
        }
    }

    pub fn get_statistics<'a>(&'a self, skipped_stats: &[String]) -> Vec<Statistic<'a>> {
        let mut statistics = vec![
            Statistic::Bucket(BucketsStatistic::new(
                "Cmd to completion",
                &self.cmd_to_completion,
                self.clock_period,
                "Red",
                "Number of clock cycles from issuing a command to receving a reponse.",
            )),
            Statistic::Bucket(BucketsStatistic::new(
                "Cmd to first data",
                &self.cmd_to_first_data,
                self.clock_period,
                "Blue",
                "Number of clock cycles from issuing a command to first data being transfered.",
            )),
            Statistic::Bucket(BucketsStatistic::new(
                "Last data to completion",
                &self.last_data_to_completion,
                self.clock_period,
                "Green",
                "Number of clock cycles from last data being transfered to transaction end.",
            )),
            Statistic::Bucket(BucketsStatistic::new(
                "Transaction delays",
                &self.transaction_delays,
                self.clock_period,
                "Pink",
                "Delays between transactions in clock cycles",
            )),
        ];
        if !skipped_stats.iter().any(|s| s == "error_rate") {
            statistics.push(Statistic::Timeline(TimelineStatistic {
                name: "Error rate [%]",
                values: vec![],
                vertical_lines: vec![], // TODO show times when error occured
                display: if self.error_rate.is_nan() {
                    "Invalid".to_string()
                } else {
                    format!("{:.2}", self.error_rate * 100.0)
                },
                description: "Percentage of transactions that resulted in error.",
            }));
        }
        statistics.push(Statistic::Timeline(TimelineStatistic {
            name: "Bandwidth [t/clk]",
            values: self.bandwidth_windows.clone(),
            vertical_lines: self
                .intervals
                .iter()
                .flat_map(|&[a, b]| [a as f64, b as f64])
                .collect(),
            display: format!("{:.4}", self.averaged_bandwidth),
            description: "Averaged bandwidth in transactions per clock cycle.",
        }));
        statistics.push(Statistic::Timeline(TimelineStatistic {
            name: "Bandwidth above x rate [%]",
            values: vec![
                [0.0, self.x_rate],
                [
                    self.bandwidth_windows.last().unwrap_or(&[0.0, 0.0])[0],
                    self.x_rate,
                ],
            ],
            vertical_lines: vec![],
            display: format!("{:.2}", self.bandwidth_above_x_rate * 100.0),
            description: "Percentage value of time during which bandwidth was higher than x rate.",
        }));
        statistics.push(Statistic::Timeline(TimelineStatistic {
            name: "Bandwidth below y rate [%]",
            values: vec![
                [0.0, self.y_rate],
                [
                    self.bandwidth_windows.last().unwrap_or(&[0.0, 0.0])[0],
                    self.y_rate,
                ],
            ],
            vertical_lines: vec![],
            display: format!("{:.2}", self.bandwidth_below_y_rate * 100.0),
            description: "Percentage value of time during which bandwidth was smaller than y rate.",
        }));
        statistics
    }

    /// Updates statistics given new transaction. When all transactions are added you should call end() to finish calculation of statistics.
    pub fn add_transaction(
        &mut self,
        time: RealTime,
        resp_time: RealTime,
        last_write: RealTime,
        first_data: RealTime,
        resp: &str,
        next: RealTime,
    ) {
        self.cmd_to_completion
            .push(Period::new(time, resp_time, self.clock_period));
        self.cmd_to_first_data
            .push(Period::new(time, first_data, self.clock_period));
        self.last_data_to_completion
            .push(Period::new(last_write, resp_time, self.clock_period));
        if resp.ends_with("00") || resp.ends_with("01") {
            self.correct_num += 1;
        } else {
            self.errors.push(time)
        }
        self.transaction_delays
            .push(Period::new(resp_time, next, self.clock_period));
    }

    pub fn add_time(&mut self, time: RealTime) {
        self.time += time;
    }

    fn transaction_coverage_in_window(&self, period: Period, window_start: u64) -> f32 {
        let (start, end) = (period.start(), period.end());
        let win_start = window_start;
        let win_end = window_start + self.window_length as u64 * self.clock_period;
        if start == end {
            if win_start < start && start < win_end {
                1.0
            } else {
                0.0
            }
        } else {
            (win_end.min(end).saturating_sub(win_start.max(start))) as f32 / (end - start) as f32
        }
    }

    /// Finishes calculation of statistics and makes sure that all temporary values are already taken into account
    // TODO: maybe we should split this struct in two as we should with SingleChannelBusUsage
    pub fn end(&mut self, time_in_reset: u32, intervals: Vec<[u64; 2]>) {
        let error_num = self.errors.len() as u32;
        self.error_rate = error_num as f32 / (self.correct_num + error_num) as f32;
        self.averaged_bandwidth = self.cmd_to_first_data.len() as f32
            / (self.time - time_in_reset as u64 * self.clock_period) as f32
            * self.clock_period as f32;

        for [start, end] in intervals.iter() {
            for i in (*start..*end + self.window_length as u64 * self.clock_period / 2)
                .step_by((self.window_length as u64 / 2 * self.clock_period) as usize)
            {
                let num: f32 = self
                    .cmd_to_completion
                    .iter()
                    .map(|t| self.transaction_coverage_in_window(*t, i))
                    .sum();
                self.bandwidth_windows
                    .push([i as f64, num as f64 / self.window_length as f64]);
            }
        }

        self.bandwidth_above_x_rate = self
            .bandwidth_windows
            .iter()
            .map(|[_, b]| b)
            .filter(|&b| *b > self.x_rate)
            .count() as f32
            / self.bandwidth_windows.len() as f32;

        self.bandwidth_below_y_rate = self
            .bandwidth_windows
            .iter()
            .map(|[_, b]| b)
            .filter(|&b| *b < self.y_rate)
            .count() as f32
            / self.bandwidth_windows.len() as f32;

        self.intervals = intervals;
    }
}
