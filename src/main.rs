use busperf::*;
use std::{env, io::Write};

struct Args {
    simulation_trace: String,
    bus_description: String,
    max_burst_delay: u32,
    verbose: bool,
    output: Option<String>,
    output_type: OutputType,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

impl Args {
    fn get_output_type() -> OutputType {
        loop {
            let mut buffer = String::new();
            print!("Output type: ");
            std::io::stdout().flush().expect("Flush should not fail");
            std::io::stdin()
                .read_line(&mut buffer)
                .expect("Failed to read output type");
            match OutputType::try_from(buffer.as_str().trim_end()) {
                Ok(output) => return output,
                Err(e) => println!("{e}"),
            }
        }
    }
    pub fn parse() -> Args {
        match Args::parse_internal() {
            Ok(args) => args,
            Err(e) => {
                println!("Failed to parse arguments: {e}");
                std::process::exit(1);
            }
        }
    }
    pub fn parse_internal() -> Result<Args, Box<dyn std::error::Error>> {
        let mut trace = Err("Needs simulation trace");
        let mut desc = Err("Needs bus description");
        let mut max_burst_delay = 0;
        let mut window_length = 10000;
        let mut x_rate = 0.0001;
        let mut y_rate = 0.00001;
        let mut verbose = false;
        let mut output = None;
        let mut output_type = None;
        let mut args = env::args();
        args.next();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-o" | "--output" => match args.next() {
                    Some(a) => output = Some(a),
                    None => Err("Expected filename after -o")?,
                },
                "--csv" => output_type = Some(OutputType::Csv),
                "--md" => output_type = Some(OutputType::Md),
                "--gui" => output_type = Some(OutputType::Rendered),
                "--text" => output_type = Some(OutputType::Pretty),
                "-m" | "--max-burst-delay" => match args.next() {
                    Some(a) => {
                        max_burst_delay = a.parse()?;
                    }
                    None => Err("Expected burst delay")?,
                },
                "-w" | "--window" => match args.next() {
                    Some(a) => window_length = a.parse()?,
                    None => Err("Expected window length")?,
                },
                "-x" | "--x_rate" => match args.next() {
                    Some(a) => x_rate = a.parse()?,
                    None => Err("Expected x_rate")?,
                },
                "-y" | "--y_rate" => match args.next() {
                    Some(a) => y_rate = a.parse()?,
                    None => Err("Expected y_rate")?,
                },
                "-v" | "--verbose" => {
                    verbose = true;
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: busperf [OPTIONS] [-t | --trace] <SIMULATION_TRACE> [-b | --bus-config] <BUS_DESCRIPTION>

Arguments:
  <SIMULATION_TRACE>                       vcd/fst file with simulation trace
  <BUS_DESCRIPTION>                        yaml with description of buses

Options:
  -o, --output <OUTPUT_FILENAME>
      --csv                                Format output as csv
      --md                                 Format output as md table
      --gui                                Run GUI
      --text                               Format output as table
  -m, --max-burst-delay <MAX_BURST_DELAY>  [default: 0]
  -w, --window <WINDOW_SIZE>               Set size of the rolling window [default: 10000]
  -x, --x-rate <VALUE>                     Set x_rate for bandwidth above x_rate [default: 0.0001]
  -y, --y-rate <VALUE>                     Set y_rate for bandwidth below y_rate [default: 0.00001]
  -t, --trace <SIMULATION_TRACE>           Path to the trace file. Can be specified as option or a positional argument
  -b, --bus-config <BUS_DESCRIPTION>       Path to the bus description yaml. Can be specified as option or a positional argument
  -v, --verbose                            
  -h, --help                               Print help"
                    );
                    std::process::exit(0);
                }
                "-t" | "--trace" => match args.next() {
                    Some(a) => trace = Ok(a.to_owned()),
                    None => {
                        Err("Missing simulation trace filename")?;
                    }
                },
                "-b" | "--bus-config" => match args.next() {
                    Some(a) => desc = Ok(a.to_owned()),
                    None => {
                        Err("Missing bus description filename")?;
                    }
                },
                arg => {
                    if trace.is_err() {
                        trace = Ok(arg.to_owned());
                    } else if desc.is_err() {
                        desc = Ok(arg.to_owned());
                    } else {
                        Err(format!("Unknown argument {arg}"))?;
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
            output_type: output_type.unwrap_or_else(Args::get_output_type),
            window_length,
            x_rate,
            y_rate,
        })
    }
}

fn main() {
    let args = Args::parse();
    let mut data = load_simulation_trace(&args.simulation_trace, args.verbose);
    let mut analyzers = load_bus_analyzers(
        &args.bus_description,
        args.max_burst_delay as i32,
        args.window_length,
        args.x_rate,
        args.y_rate,
    )
    .unwrap();
    for a in analyzers.iter_mut() {
        a.analyze(&mut data, args.verbose);
    }
    let mut out: &mut dyn std::io::Write = match args.output {
        None => &mut std::io::stdout(),
        Some(filename) => &mut std::fs::File::create(filename).unwrap(),
    };
    show_data(
        analyzers,
        args.output_type,
        Some(&mut out),
        &mut data,
        &args.simulation_trace,
        args.verbose,
    );
}
