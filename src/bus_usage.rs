use crate::{
    bus::{CyclesNum, DelaysNum},
    CycleType,
};

#[derive(PartialEq, Debug)]
pub enum BusUsage {
    SingleChannel(SingleChannelBusUsage),
    MultiChannel(MultiChannelBusUsage),
}

#[derive(PartialEq, Debug)]
pub struct SingleChannelBusUsage {
    pub bus_name: String,
    busy: CyclesNum,
    backpressure: CyclesNum,
    no_data: CyclesNum,
    no_transaction: CyclesNum,
    free: CyclesNum,
    reset: CyclesNum,
    transaction_delays: Vec<CyclesNum>,
    current_delay: usize,
    transaction_delay_buckets: Vec<DelaysNum>,
    burst_lengths: Vec<CyclesNum>,
    burst_length_buckets: Vec<u32>,
    burst_delays: CyclesNum,
    current_burst: usize,
    max_burst_delay: CyclesNum,
}
impl SingleChannelBusUsage {
    pub(crate) fn new(name: &str, max_burst_delay: CyclesNum) -> SingleChannelBusUsage {
        SingleChannelBusUsage {
            bus_name: name.to_owned(),
            busy: 0,
            backpressure: 0,
            no_data: 0,
            no_transaction: 0,
            free: 0,
            reset: 0,
            transaction_delays: vec![0],
            current_delay: 0,
            transaction_delay_buckets: vec![],
            burst_lengths: vec![0],
            current_burst: 0,
            burst_length_buckets: vec![],
            burst_delays: 0,
            max_burst_delay,
        }
    }

    pub(crate) fn add_cycle(&mut self, t: CycleType) {
        if let CycleType::Busy = t {
            self.add_busy_cycle();
        } else {
            self.add_wasted_cycle(t);
        }
    }

    fn add_busy_cycle(&mut self) {
        self.busy += 1;
        self.burst_lengths[self.current_burst] += 1;

        let transaction_delay = self.transaction_delays[self.current_delay];
        if transaction_delay > 0 {
            if transaction_delay > self.max_burst_delay {
                self.transaction_delays.push(0);
                self.current_delay += 1;
                let bucket = transaction_delay.ilog2() as usize;
                if self.transaction_delay_buckets.len() <= bucket {
                    self.transaction_delay_buckets.resize(bucket + 1, 0);
                }
                self.transaction_delay_buckets[bucket] += 1;
            } else {
                self.burst_delays += transaction_delay;
            }
            self.transaction_delays[self.current_delay] = 0;
        }
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
        self.transaction_delays[self.current_delay] += 1;
        let transaction_delay = self.transaction_delays[self.current_delay];
        let burst_length = self.burst_lengths[self.current_burst];
        if transaction_delay > self.max_burst_delay {
            if burst_length > transaction_delay - 1 {
                let actual_length = burst_length - self.max_burst_delay;
                self.burst_lengths[self.current_burst] -= self.max_burst_delay;
                self.burst_lengths.push(0);
                self.current_burst += 1;
                let bucket = actual_length.ilog2() as usize;
                if self.burst_length_buckets.len() <= bucket {
                    self.burst_length_buckets.resize(bucket + 1, 0);
                }
                self.burst_length_buckets[bucket] += 1;
            }
            self.burst_lengths[self.current_burst] = 0;
        } else {
            self.burst_lengths[self.current_burst] += 1;
        }
    }

    pub(crate) fn end(&mut self) {
        let burst_length = self.burst_lengths[self.current_burst];
        if burst_length > 0 {
            let bucket = burst_length.ilog2() as usize;
            if self.burst_length_buckets.len() <= bucket {
                self.burst_length_buckets.resize(bucket + 1, 0);
            }
            self.burst_length_buckets[bucket] += 1;
        } else {
            self.burst_lengths.pop();
        }
        let transaction_delay = self.transaction_delays[self.current_delay];
        if transaction_delay > 0 {
            let bucket = transaction_delay.ilog2() as usize;
            if self.transaction_delay_buckets.len() <= bucket {
                self.transaction_delay_buckets.resize(bucket + 1, 0);
            }
            self.transaction_delay_buckets[bucket] += 1;
        } else {
            self.transaction_delays.pop();
        }
    }

    pub fn get_data(&self, delays_num: usize, bursts_num: usize, verbose: bool) -> Vec<String> {
        let time =
            (self.busy + self.backpressure + self.no_data + self.free + self.no_transaction) as f32;
        let mut v = vec![
            self.bus_name.to_string(),
            format!("{}({:.2})", self.busy, self.busy as f32 / time * 100.0),
            format!(
                "{}({:.2})",
                self.no_transaction,
                self.no_transaction as f32 / time * 100.0
            ),
            format!(
                "{}({:.2})",
                self.backpressure,
                self.backpressure as f32 / time * 100.0
            ),
            format!(
                "{}({:.2})",
                self.no_data,
                self.no_data as f32 / time * 100.0
            ),
            format!("{}({:.2})", self.free, self.free as f32 / time * 100.0),
        ];

        v.push(if verbose {
            format!("{:?}", self.transaction_delays)
        } else {
            String::from("")
        });

        for num in self.transaction_delay_buckets.iter() {
            v.push(num.to_string());
        }
        for _ in 0..delays_num - self.transaction_delay_buckets.len() {
            v.push(String::from("0"));
        }

        v.push(if verbose {
            format!("{:?}", self.burst_lengths)
        } else {
            String::from("")
        });

        for num in self.burst_length_buckets.iter() {
            v.push(num.to_string());
        }
        for _ in 0..bursts_num - self.burst_length_buckets.len() {
            v.push(String::from("0"));
        }
        v
    }

    pub fn literal(
        bus_name: &str,
        busy: CyclesNum,
        backpressure: CyclesNum,
        no_data: CyclesNum,
        no_transaction: CyclesNum,
        free: CyclesNum,
        reset: CyclesNum,
        transaction_delays: Vec<CyclesNum>,
        current_delay: usize,
        transaction_delay_buckets: Vec<DelaysNum>,
        burst_lengths: Vec<CyclesNum>,
        burst_length_buckets: Vec<u32>,
        burst_delays: CyclesNum,
        current_burst: usize,
        max_burst_delay: CyclesNum,
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
            current_delay,
            transaction_delay_buckets,
            burst_lengths,
            burst_length_buckets,
            burst_delays,
            current_burst,
            max_burst_delay,
        }
    }
}

pub fn get_header(usages: &[&SingleChannelBusUsage]) -> (Vec<String>, usize, usize) {
    let delays = usages
        .iter()
        .map(|u| u.transaction_delay_buckets.len())
        .max()
        .unwrap();
    let bursts = usages
        .iter()
        .map(|u| u.burst_length_buckets.len())
        .max()
        .unwrap();

    let mut v = vec![
        String::from("bus_name"),
        String::from("busy(%)"),
        String::from("no transaction(%)"),
        String::from("backpressure(%)"),
        String::from("no data to send(%)"),
        String::from("free(%)"),
        String::from("delays between transactions "),
    ];
    for i in 0..delays {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }
    v.push(format!("burst lengths"));
    for i in 0..bursts {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1));
    }
    (v, delays, bursts)
}

pub fn get_header_multi(usages: &[&MultiChannelBusUsage]) -> (Vec<String>, u32, u32, u32, u32) {
    let mut v = vec![String::from("bus_name"), String::from("cmd_to_completion")];
    let max1 = usages
        .iter()
        .map(|u| u.cmd_to_completion.buckets_num())
        .max()
        .unwrap();
    v.push(String::from("0-0"));
    for i in 0..max1 - 1 {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }
    v.push(String::from("cmd_to_first_data"));
    let max2 = usages
        .iter()
        .map(|u| u.cmd_to_first_data.buckets_num())
        .max()
        .unwrap();
    v.push(String::from("0-0"));
    for i in 0..max2 - 1 {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }
    v.push(String::from("last_data_to_completion"));
    let max3 = usages
        .iter()
        .map(|u| u.last_data_to_completion.buckets_num())
        .max()
        .unwrap();
    v.push(String::from("0-0"));
    for i in 0..max3 - 1 {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }
    v.push(String::from("transaction delays"));
    let max4 = usages
        .iter()
        .map(|u| u.transaction_delays.buckets_num())
        .max()
        .unwrap();
    v.push(String::from("0-0"));
    for i in 0..max4 - 1 {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }

    v.append(&mut vec![
        String::from("error rate"),
        String::from("averaged_bandwidth"),
        String::from("bandwidth_above_x_rate"),
        String::from("bandwidth_below_y_rate"),
    ]);
    (v, max1, max2, max3, max4)
}

#[derive(PartialEq, Debug)]
pub struct VecStatistic {
    name: &'static str,
    data: Vec<CyclesNum>,
}

impl VecStatistic {
    pub fn new(name: &'static str) -> VecStatistic {
        VecStatistic { name, data: vec![] }
    }
    pub fn get_buckets(&self) -> Vec<usize> {
        let mut buckets = vec![];
        for v in self.data.iter() {
            let bucket = if *v == 0 { 0 } else { (v.ilog2() + 1) as usize };
            if buckets.len() <= bucket {
                buckets.resize(bucket + 1, 0);
            }
            buckets[bucket] += 1;
        }
        buckets
    }

    pub fn buckets_num(&self) -> u32 {
        if *self.data.iter().max().unwrap() == 0 {
            return 1;
        }
        self.data.iter().max().unwrap().ilog2() + 2
    }

    pub fn add(&mut self, value: CyclesNum) {
        self.data.push(value);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[derive(PartialEq, Debug)]
pub struct MultiChannelBusUsage {
    pub bus_name: String,
    cmd_to_completion: VecStatistic,
    cmd_to_first_data: VecStatistic,
    last_data_to_completion: VecStatistic,
    transaction_delays: VecStatistic,
    transaction_times: Vec<(u32, u32)>,
    error_rate: f32,
    error_num: u32,
    correct_num: u32,
    averaged_bandwidth: f32,
    bandwidth_windows: Vec<f32>,
    window_length: u32,
    bandwidth_above_x_rate: f32,
    bandwidth_below_y_rate: f32,
    pub channels_usages: Vec<SingleChannelBusUsage>,
    time: u32,
    x_rate: f32,
    y_rate: f32,
}

impl MultiChannelBusUsage {
    pub fn new(bus_name: &str, window_length: u32, x_rate: f32, y_rate: f32) -> Self {
        MultiChannelBusUsage {
            bus_name: bus_name.to_owned(),
            cmd_to_completion: VecStatistic::new("cmd to completion"),
            cmd_to_first_data: VecStatistic::new("cmd to first data"),
            last_data_to_completion: VecStatistic::new("last data to completion"),
            transaction_delays: VecStatistic::new("transaction_delays"),
            transaction_times: vec![],
            error_rate: 0.0,
            error_num: 0,
            correct_num: 0,
            averaged_bandwidth: 0.0,
            bandwidth_windows: vec![],
            window_length,
            bandwidth_above_x_rate: 0.0,
            bandwidth_below_y_rate: 0.0,
            channels_usages: vec![],
            time: 0,
            x_rate,
            y_rate,
        }
    }

    pub fn add_transaction(
        &mut self,
        time: u32,
        resp_time: u32,
        last_write: u32,
        first_data: u32,
        resp: &str,
        delay: u32,
    ) {
        self.cmd_to_completion.add((resp_time - time) / 2);
        self.cmd_to_first_data.add((first_data - time) / 2);
        self.last_data_to_completion
            .add((resp_time - last_write) / 2);
        if resp.ends_with("00") || resp.ends_with("01") {
            self.correct_num += 1;
        } else {
            self.error_num += 1;
        }
        self.transaction_delays.add(delay);
        self.transaction_times.push((time, resp_time));
        println!("TIME {}", time);
        self.time = resp_time + delay;
    }

    fn transaction_coverage_in_window(
        &self,
        (start, end): (u32, u32),
        window_num: u32,
        offset: u32,
    ) -> f32 {
        let win_start = window_num * self.window_length + offset;
        let win_end = (window_num + 1) * self.window_length + offset;
        // println!("win {} {} tran {} {}", win_start, win_end, start, end);

        (win_end.min(end).saturating_sub(win_start.max(start))) as f32 / (end - start) as f32
    }

    pub fn end(&mut self) {
        self.error_rate = self.error_num as f32 / self.correct_num as f32;
        self.averaged_bandwidth = self.cmd_to_first_data.len() as f32
            / (self.time - self.channels_usages[0].reset) as f32;

        println!("{}", self.time);
        for i in 0..(self.time / self.window_length) {
            let half = self.window_length / 2;
            let num: f32 = self
                .transaction_times
                .iter()
                .map(|t| self.transaction_coverage_in_window(*t, i, 0))
                // .inspect(|f| println!("FOO {}", f))
                .sum();
            self.bandwidth_windows
                .push(num as f32 / self.window_length as f32);
            // println!(
            //     "{}-{}: {}",
            //     self.window_length * i,
            //     self.window_length * (i + 1),
            //     num
            // );

            let num: f32 = self
                .transaction_times
                .iter()
                .map(|t| self.transaction_coverage_in_window(*t, i, half))
                // .inspect(|f| println!("BOO {}", f))
                .sum();
            self.bandwidth_windows
                .push(num as f32 / self.window_length as f32);
        }
        println!("{:?}", self.bandwidth_windows);

        self.bandwidth_above_x_rate = self
            .bandwidth_windows
            .iter()
            .filter(|&b| *b > self.x_rate)
            .count() as f32
            / self.bandwidth_windows.len() as f32;

        self.bandwidth_below_y_rate = self
            .bandwidth_windows
            .iter()
            .filter(|&b| *b < self.y_rate)
            .count() as f32
            / self.bandwidth_windows.len() as f32;
    }

    pub fn get_data(
        &self,
        verbose: bool,
        c2c: u32,
        c2d: u32,
        ld2c: u32,
        delays: u32,
    ) -> Vec<String> {
        let mut v = vec![self.bus_name.clone()];
        v.push(if verbose {
            format!("{:?}", self.cmd_to_completion.data)
        } else {
            String::from("")
        });
        for num in self.cmd_to_completion.get_buckets().iter() {
            v.push(num.to_string());
        }
        for _ in 0..c2c - self.cmd_to_completion.buckets_num() {
            v.push(String::from("0"));
        }
        v.push(if verbose {
            format!("{:?}", self.cmd_to_first_data.data)
        } else {
            String::from("")
        });
        for num in self.cmd_to_first_data.get_buckets().iter() {
            v.push(num.to_string());
        }
        for _ in 0..c2d - self.cmd_to_first_data.buckets_num() {
            v.push(String::from("0"));
        }
        v.push(if verbose {
            format!("{:?}", self.last_data_to_completion.data)
        } else {
            String::from("")
        });
        for num in self.last_data_to_completion.get_buckets().iter() {
            v.push(num.to_string());
        }
        for _ in 0..ld2c - self.last_data_to_completion.buckets_num() {
            v.push(String::from("0"));
        }
        v.push(if verbose {
            format!("{:?}", self.transaction_delays.data)
        } else {
            String::from("")
        });
        for num in self.transaction_delays.get_buckets().iter() {
            v.push(num.to_string());
        }
        for _ in 0..delays - self.transaction_delays.buckets_num() {
            v.push(String::from("0"));
        }
        v.push(self.error_rate.to_string());
        v.push(self.averaged_bandwidth.to_string());
        v.push(self.bandwidth_above_x_rate.to_string());
        v.push(self.bandwidth_below_y_rate.to_string());

        v
    }
}
