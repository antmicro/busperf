use std::{collections::HashMap, error::Error};

use itertools::Itertools;
use libbusperf::bus_usage::RealTime;
use yaml_rust2::Yaml;

use crate::analyze::trigger::{Control, TriggerName};

type StateName = String;

pub struct Fsm {
    name: String,
    default_state: String,
    states: HashMap<StateName, State>,
}

struct State {
    transitions: Vec<(TriggerName, StateName)>,
    epsilon_transition: Option<StateName>,
    trigger: Option<TriggerName>,
}

impl Fsm {
    pub fn build_from_yaml(machine_name: String, yaml: &Yaml) -> Result<Self, Box<dyn Error>> {
        let states = yaml["states"].as_hash().ok_or("States are not defined")?;
        let default_state = states
            .front()
            .ok_or("No states defined")?
            .0
            .as_str()
            .ok_or("invalid state name")?
            .to_owned();
        let states = states
            .iter()
            .map(|(name, yaml)| {
                let name = name.as_str().ok_or("invalid state name")?.to_owned();
                let mut epsilon_transition = None;
                let transitions =
                        match &yaml["transition_to"] {
                            Yaml::Hash(linked_hash_map) => linked_hash_map
                            .iter()
                            .map(
                                |(to_state, triggers)| -> Result<
                                    Vec<(TriggerName, StateName)>,
                                    Box<dyn Error>,
                                > {
                                    let to_state = to_state
                                        .as_str()
                                        .ok_or("transition target invalid")?
                                        .to_owned();
                                    match triggers {
                                        Yaml::String(trigger_name) => Ok(vec![(
                                            trigger_name.to_owned(),
                                            to_state,
                                        )]),
                                        Yaml::Array(yamls) => {
                                            if yamls.is_empty() {
                                                epsilon_transition = Some(to_state);
                                                Ok(vec![])
                                            } else {
                                                Ok(yamls
                                                    .iter()
                                                    .map(|y| y.as_str().ok_or("invalid trigger"))
                                                    .collect::<Result<Vec<_>, _>>()?
                                                    .into_iter()
                                                    .map(|trigger| {
                                                        (
                                                            trigger.to_owned(),
                                                            to_state.clone(),
                                                        )
                                                    })
                                                    .collect())
                                            }
                                        }
                                        _ => Err("invalid triggers")?,
                                    }
                                },
                            )
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .flatten()
                            .collect(),
                            Yaml::BadValue => vec![],
                            other => Err(format!("transitions badly defined, {other:?}"))?
                        };
                let trigger = match &yaml["trigger_set"] {
                    Yaml::String(s) => {
                        Some(format!("control_only.{}.{}", machine_name, s.to_owned()))
                    }
                    Yaml::BadValue => None,
                    other => Err(format!("invalid trigger name: {:?}", other))?,
                };
                Ok((
                    name,
                    State {
                        transitions,
                        trigger,
                        epsilon_transition,
                    },
                ))
            })
            .collect::<Result<HashMap<StateName, State>, Box<dyn Error>>>()?;

        // Check if all states used in transitions are defined
        for (name, state) in states.iter() {
            for (_, state_name) in state.transitions.iter() {
                if !states.contains_key(state_name) {
                    Err(format!(
                        "state {name} defines transition to {state_name} which does not exist"
                    ))?;
                }
            }
        }

        Ok(Self {
            name: machine_name,
            states,
            default_state,
        })
    }
}

impl Control for Fsm {
    fn requires(&self) -> Vec<&str> {
        self.states
            .values()
            .flat_map(|s| s.transitions.iter().map(|(t, _)| t.as_str()))
            .collect()
    }

    fn names(&self) -> Vec<&str> {
        self.states
            .values()
            .filter_map(|s| s.trigger.as_deref())
            .collect()
    }

    fn analyze(
        self: Box<Self>,
        done_triggers: &crate::analyze::DoneTriggers,
    ) -> Vec<(
        String,
        Result<Vec<libbusperf::bus_usage::RealTime>, Box<dyn std::error::Error>>,
    )> {
        let triggers = self
            .states
            .iter()
            .map(|(s_name, state)| {
                let transitions = state
                    .transitions
                    .iter()
                    .map(|(trigger_name, to_state)| {
                        Ok(done_triggers[trigger_name]
                            .as_ref()?
                            .iter()
                            .map(move |time| (time, to_state)))
                    })
                    .collect::<Result<Vec<_>, &Box<dyn Error>>>()?;
                let transitions = transitions
                    .into_iter()
                    .kmerge_by(|(time1, _), (time2, _)| time1 < time2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .peekable();
                Ok((s_name, transitions))
            })
            .collect::<Result<HashMap<&StateName, _>, &Box<dyn Error>>>()
            .map_err(|e| format!("{e}"));
        match triggers {
            Ok(mut triggers) => {
                let mut current_state = &self.default_state;
                let mut current_time = 0;
                let mut times_triggered: HashMap<StateName, Vec<RealTime>> = HashMap::new();
                for (_, state) in self.states.iter() {
                    if let Some(trigger) = &state.trigger {
                        times_triggered.insert(trigger.clone(), vec![]);
                    }
                }

                loop {
                    let trigger = triggers
                        .get_mut(current_state)
                        .expect("each state should has been processed");
                    while trigger.next_if(|&(t, _)| *t < current_time).is_some() {}
                    let (time, to_state) = if let Some((time, to_state)) = trigger.peek().copied() {
                        if let Some(transition) = &self.states[current_state].epsilon_transition
                            && *time > current_time
                        {
                            (&current_time, transition)
                        } else {
                            trigger.next();
                            (time, to_state)
                        }
                    } else if let Some(transition) = &self.states[current_state].epsilon_transition
                    {
                        (&current_time, transition)
                    } else {
                        return times_triggered
                            .into_iter()
                            .map(|(name, v)| (name, Ok(v)))
                            .collect();
                    };

                    current_state = to_state;
                    current_time = *time;
                    if let Some(trigger) = &self.states[current_state].trigger {
                        times_triggered
                            .get_mut(trigger)
                            .expect("Hashmap has been filled with all trigger names")
                            .push(current_time);
                    }
                }
            }
            Err(e) => self
                .states
                .into_iter()
                .filter_map(|(_, state)| {
                    state
                        .trigger
                        .map(|t| (t, Err(format!("used trigger failed: {e}").into())))
                })
                .collect(),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
