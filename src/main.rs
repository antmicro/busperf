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

use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

fn main() {
    Python::with_gil(|py| {
        let sys = py.import("sys").unwrap();
        let version: String = sys.getattr("version").unwrap().extract().unwrap();

        let locals = [("os", py.import("os").unwrap())].into_py_dict(py).unwrap();
        let code = c_str!("os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'");
        let user: String = py
            .eval(code, None, Some(&locals))
            .unwrap()
            .extract()
            .unwrap();

        println!("Hello {}, I'm Python {}", user, version);
    });

    let py_foo = c_str!(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/python/utils/foo.py"
    )));
    let py_app = c_str!(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/python/app.py"
    )));
    let from_python = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
        PyModule::from_code(py, py_foo, c_str!("utils.foo"), c_str!("utils.foo"))?;
        Into::<Py<PyAny>>::into(
            PyModule::from_code(py, py_app, c_str!(""), c_str!(""))?.getattr("run")?,
        )
        .call0(py)
    });
    println!("py: {}", from_python.unwrap());

    let args = Args::parse();
    let mut data = load_simulation_trace(&args.simulation_trace);
    let descs = load_bus_descriptions(&args.bus_description, args.max_burst_delay).unwrap();
    let usages: Vec<BusUsage> = descs
        .iter()
        .map(|d| calculate_usage(&mut data, &**d))
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
