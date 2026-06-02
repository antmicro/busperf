use std::{error::Error, rc::Rc};

use crate::analyze::{
    AnalyzersConfig, SignalValue, TimeTable,
    bus::{
        BusDescription, LockstepAnalyzer, SignalPath, custom_python::PythonCustomBus,
        is_value_of_type,
    },
    trigger::{ChannelTrigger, PythonTrigger, TriggerName, TriggerSink, TriggerSource},
};
use libbusperf::bus_usage::{BusUsage, RealTime, SingleChannelBusUsage};
use libbusperf::{CycleType, CyclesNum};

use super::Analyzer;

pub struct PythonBusAnalyzer {
    bus: Rc<PythonCustomBus>,
    max_burst_delay: CyclesNum,

    required: Vec<TriggerName>,
    sink: TriggerSink,
    provided: Vec<Box<dyn TriggerSource>>,
    provided_python: Vec<PythonTrigger>,
}

impl PythonBusAnalyzer {
    pub fn from_yaml(
        name: String,
        yaml: yaml_rust2::Yaml,
        config: &AnalyzersConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let handshake = yaml["custom_handshake"]
            .as_str()
            .ok_or("Custom bus has to specify handshake interpreter")?;
        let bus = Rc::new(PythonCustomBus::from_yaml(
            handshake,
            name,
            &yaml,
            &config.plugins_path,
        )?);
        let sink = TriggerSink::build_from_yaml(&yaml)?;
        let required = sink.required();
        let provided = match &yaml["triggers"] {
            yaml_rust2::Yaml::Hash(hash) => hash
                .iter()
                .map(|(name, yaml)| {
                    let name = format!(
                        "interfaces.{}.{}",
                        bus.name(),
                        name.as_str().ok_or("invalid trigger name")?.to_owned()
                    );
                    let clk_path = bus.get_by_name("clock").expect("Should have clock");
                    ChannelTrigger::from_yaml(
                        name,
                        yaml,
                        &(Rc::clone(&bus) as Rc<dyn BusDescription>),
                        &(Rc::clone(&bus) as Rc<dyn LockstepAnalyzer>),
                        clk_path,
                    )
                })
                .collect::<Result<_, _>>()?,
            yaml_rust2::Yaml::BadValue => vec![],
            _ => Err("bad triggers")?,
        };

        let provided_python = bus.provides()?;

        Ok(PythonBusAnalyzer {
            bus,
            max_burst_delay: config.default_max_burst_delay,
            required,
            sink,
            provided,
            provided_python,
        })
    }
}

impl Analyzer for PythonBusAnalyzer {
    fn bus_name(&self) -> &str {
        self.bus.name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        self.bus.get_signals()
    }

    fn calculate(
        &mut self,
        loaded: &[&wellen::Signal],
        intervals: &[[RealTime; 2]],
        time_table: &TimeTable,
    ) -> Result<BusUsage, Box<dyn std::error::Error + 'static>> {
        let clock = loaded[0];
        let reset = loaded[1];
        let mut usage = SingleChannelBusUsage::new(
            self.bus.name(),
            self.max_burst_delay,
            *time_table.get(2).ok_or(
                "trace is too short (less than 3 time indices), cannot calculate clock period",
            )?,
        );
        let mut intervals = intervals.iter();
        if let Some(mut current_iterval) = intervals.next() {
            for (clk_time, value) in clock.iter_changes() {
                if is_value_of_type(value, crate::analyze::bus::ValueType::V0) {
                    continue;
                }
                let t = time_table[clk_time as usize];
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
                let time = clk_time.saturating_sub(1);
                let reset = reset.get_value_at(
                    &reset.get_offset(time).ok_or(format!(
                        "reset value is invalid at {}",
                        time_table[time as usize]
                    ))?,
                    0,
                );
                let values: Vec<SignalValue> = loaded[2..]
                    .iter()
                    .map(|s| {
                        Ok::<_, Box<dyn Error>>(s.get_value_at(
                            &s.get_offset(time).ok_or(format!(
                                "signal does not have value at {}",
                                time_table[time as usize]
                            ))?,
                            0,
                        ))
                    })
                    .collect::<Result<_, _>>()?;

                for trigger_name in self.bus.get_triggers(&values)? {
                    self.provided_python
                        .iter_mut()
                        .find(|p| {
                            p.name() == format!("interfaces.{}.{}", self.bus.name(), trigger_name)
                        })
                        .ok_or("plugin returned trigger that it did not define")?
                        .add_time(time_table[clk_time as usize]);
                }
                if !is_value_of_type(reset, self.bus.common().rst_active_value()) {
                    let type_ = self.bus.interpret_cycle(&values, time);
                    if let CycleType::Unknown = type_ {
                        let mut state = String::new();
                        self.bus
                            .get_signals()
                            .iter()
                            .zip(values)
                            .for_each(|(name, value)| {
                                state.push_str(&format!("{name}: {value}, "))
                            });
                        eprintln!(
                            "[WARN] bus \"{}\" in unknown state outside reset at time: {} - {}",
                            self.bus.name(),
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
        self.provided
            .iter()
            .map(|p| p.name())
            .chain(self.provided_python.iter().map(|p| p.name()))
            .collect()
    }

    fn sink(&self) -> &crate::analyze::trigger::TriggerSink {
        &self.sink
    }

    fn consume(
        mut self: Box<Self>,
    ) -> (String, Rc<dyn BusDescription>, Vec<Box<dyn TriggerSource>>) {
        self.provided_python
            .into_iter()
            .for_each(|p| self.provided.push(Box::new(p)));
        (self.bus.name().to_owned(), self.bus, self.provided)
    }
}
