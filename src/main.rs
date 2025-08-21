use busperf::*;
use std::env;

enum OutputType {
    Pretty,
    Csv,
    Md,
}

impl TryFrom<&str> for OutputType {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pretty" => Ok(Self::Pretty),
            "csv" => Ok(Self::Csv),
            "md" => Ok(Self::Md),
            _ => Err("Expected one of [pretty, csv, md]"),
        }
    }
}

struct Args {
    simulation_trace: String,
    bus_description: String,
    max_burst_delay: u32,
    verbose: bool,
    output: Option<String>,
    output_type: OutputType,
}

impl Args {
    pub fn parse() -> Args {
        match Args::parse_internal() {
            Ok(args) => args,
            Err(e) => {
                println!("Failed to parse arguments: {}", e);
                std::process::exit(1);
            }
        }
    }
    pub fn parse_internal() -> Result<Args, Box<dyn std::error::Error>> {
        let mut trace = Err("Needs simulation trace");
        let mut desc = Err("Needs bus description");
        let mut max_burst_delay = 0;
        let mut verbose = false;
        let mut output = None;
        let mut output_type = OutputType::Pretty;
        let mut args = env::args();
        args.next();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-m" => match args.next() {
                    Some(a) => {
                        max_burst_delay = a.parse()?;
                    }
                    None => Err("Expected burst delay")?,
                },
                "-v" | "--verbose" => {
                    verbose = true;
                }
                "-o" | "--output" => match args.next() {
                    Some(a) => output = Some(a),
                    None => Err("Expected filename after -o")?,
                },
                "-t" | "--type" => match args.next() {
                    Some(a) => output_type = a.as_str().try_into()?,
                    None => {
                        TryInto::<OutputType>::try_into("")?;
                    }
                },
                "--help" | "-h" => {
                    println!(
                        "Usage: busperf [OPTIONS] <SIMULATION_TRACE> <BUS_DESCRIPTION>

Arguments:
  <SIMULATION_TRACE>  
  <BUS_DESCRIPTION>   

Options:
  -m, --max-burst-delay <MAX_BURST_DELAY>  [default: 0]
  -v, --verbose                            
  -o, --output <OUTPUT_FILENAME>           
  -t, --output-type <OUTPUT_TYPE>          [possible values: pretty[default],csv, md]
  -h, --help                               Print help"
                    );
                    std::process::exit(0);
                }
                arg => {
                    if trace.is_err() {
                        trace = Ok(arg.to_owned());
                    } else if desc.is_err() {
                        desc = Ok(arg.to_owned());
                    } else {
                        Err(format!("Unknown argument {}", arg))?;
                    }
                }
            }
        }
        Ok(Args {
            simulation_trace: trace?,
            bus_description: desc?,
            max_burst_delay,
            verbose,
            output,
            output_type,
        })
    }
}

fn main() {
    let args = Args::parse();
    let mut data = load_simulation_trace(&args.simulation_trace, args.verbose);
    let descs = load_bus_descriptions(&args.bus_description, args.max_burst_delay).unwrap();
    let usages: Vec<BusUsage> = descs
        .iter()
        .map(|d| calculate_usage(&mut data, &**d, args.verbose))
        .collect();
    let mut out: &mut dyn std::io::Write = match args.output {
        None => &mut std::io::stdout(),
        Some(filename) => &mut std::fs::File::create(filename).unwrap(),
    };
    match args.output_type {
        OutputType::Csv => generate_csv(&mut out, &usages, args.verbose),
        OutputType::Md => generate_md_table(&mut out, &usages, args.verbose),
        OutputType::Pretty => print_statistics(&mut out, &usages, args.verbose),
    }
}
