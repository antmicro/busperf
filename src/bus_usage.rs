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
            CycleType::Busy => unreachable!(),
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

#[derive(PartialEq, Debug)]
struct MultiChannelBusUsage {}
