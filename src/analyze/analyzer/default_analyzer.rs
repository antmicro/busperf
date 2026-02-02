use std::{error::Error, rc::Rc};

use wellen::{SignalValue, TimeTable};

use crate::analyze::{
    AnalyzersConfig,
    analyzer::private::AnalyzerInternal,
    bus::{BusDescription, BusDescriptionBuilder, LockstepAnalyzer, SignalPath, is_value_of_type},
    trigger::{TriggerName, TriggerSink, TriggerSource, channel_trigger::ChannelTrigger},
};
use libbusperf::bus_usage::{BusUsage, RealTime, SingleChannelBusUsage};
use libbusperf::{CycleType, CyclesNum};

use super::Analyzer;

pub struct DefaultAnalyzer {
    description: Rc<dyn BusDescription>,
    analyzer: Rc<dyn LockstepAnalyzer>,
    max_burst_delay: CyclesNum,

    required: Vec<TriggerName>,
    sink: TriggerSink,
    provided: Vec<Box<dyn TriggerSource>>,
}

impl DefaultAnalyzer {
    pub fn from_yaml(
        name: String,
        yaml: yaml_rust2::Yaml,
        config: &AnalyzersConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (description, analyzer) =
            BusDescriptionBuilder::build(name, &yaml, &config.plugins_path)?;
        let analyzer = analyzer;
        let sink = TriggerSink::build_from_yaml(&yaml)?;
        let required = sink.required();
        let provided = match &yaml["triggers"] {
            yaml_rust2::Yaml::Hash(hash) => hash
                .iter()
                .map(|(name, yaml)| {
                    let name = format!(
                        "interfaces.{}.{}",
                        description.name(),
                        name.as_str().ok_or("invalid trigger name")?.to_owned()
                    );
                    let clk_path = description.get_by_name("clock").expect("Should have clock");
                    ChannelTrigger::from_yaml(name, yaml, &description, &analyzer, clk_path)
                })
                .collect::<Result<_, _>>()?,
            yaml_rust2::Yaml::BadValue => vec![],
            _ => Err("bad triggers")?,
        };
        Ok(DefaultAnalyzer {
            description,
            analyzer,
            max_burst_delay: config.default_max_burst_delay,
            required,
            sink,
            provided,
        })
    }
}

impl AnalyzerInternal for DefaultAnalyzer {
    fn bus_name(&self) -> &str {
        self.description.name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        self.description.get_signals()
    }

    fn calculate(
        &mut self,
        loaded: &[&(wellen::SignalRef, wellen::Signal)],
        intervals: &[[RealTime; 2]],
        time_table: &TimeTable,
    ) -> Result<BusUsage, Box<dyn std::error::Error + 'static>> {
        let (_, clock) = loaded[0];
        let (_, reset) = loaded[1];
        let mut usage = SingleChannelBusUsage::new(
            self.description.name(),
            self.max_burst_delay,
            *time_table.get(2).ok_or(
                "trace is too short (less than 3 time indices), cannot calculate clock period",
            )?,
        );
        let mut intervals = intervals.iter();
        if let Some(mut current_iterval) = intervals.next() {
            for (time, value) in clock.iter_changes() {
                if let SignalValue::Binary(v, 1) = value
                    && v[0] == 0
                {
                    continue;
                }
                let t = time_table[time as usize];
                let &[mut start, end] = current_iterval;
                if t > end {
                    let Some(n) = intervals.next() else {
                        break;
                    };
                    current_iterval = n;
                    start = n[0];
                }
                if t < start {
                    usage.skip_till(t);
                    continue;
                }

                // We subtract one to use values just before clock signal
                let time = time.saturating_sub(1);
                let reset = reset.get_value_at(
                    &reset.get_offset(time).ok_or(format!(
                        "reset value is invalid at {}",
                        time_table[time as usize]
                    ))?,
                    0,
                );
                let values: Vec<SignalValue> = loaded[2..]
                    .iter()
                    .map(|(_, s)| {
                        Ok::<_, Box<dyn Error>>(s.get_value_at(
                            &s.get_offset(time).ok_or(format!(
                                "signal does not have value at {}",
                                time_table[time as usize]
                            ))?,
                            0,
                        ))
                    })
                    .collect::<Result<_, _>>()?;

                if !is_value_of_type(reset, self.description.common().rst_active_value()) {
                    let type_ = self.analyzer.interpret_cycle(&values, time);
                    if let CycleType::Unknown = type_ {
                        let mut state = String::new();
                        self.description.get_signals().iter().zip(values).for_each(
                            |(name, value)| state.push_str(&format!("{name}: {value}, ")),
                        );
                        eprintln!(
                            "[WARN] bus \"{}\" in unknown state outside reset at time: {} - {}",
                            self.description.name(),
                            time_table[time as usize],
                            state
                        );
                    }
                    usage.add_cycle(type_);
                } else {
                    usage.add_cycle(CycleType::Reset);
                }
            }
        }

        Ok(BusUsage::SingleChannel(usage))
    }

    fn requires(&self) -> Vec<&str> {
        self.required.iter().map(|n| n.as_str()).collect()
    }

    fn provides(&self) -> Vec<&str> {
        self.provided.iter().map(|p| p.name()).collect()
    }

    fn sink(&self) -> &crate::analyze::trigger::TriggerSink {
        &self.sink
    }

    fn consume(self: Box<Self>) -> (String, Rc<dyn BusDescription>, Vec<Box<dyn TriggerSource>>) {
        (
            self.description.name().to_owned(),
            self.description,
            self.provided,
        )
    }
}

impl Analyzer for DefaultAnalyzer {}
