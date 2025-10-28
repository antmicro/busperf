use bpaf::{Parser, construct, long, positional, short};
use busperf::*;

struct Args {
    files: FileArgs,
    max_burst_delay: u32,
    verbose: bool,
    output: Option<String>,
    skipped_stats: Option<String>,
    output_type: OutputType,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

struct FileArgs {
    simulation_trace: String,
    bus_description: String,
}

impl Args {
    pub fn parse() -> Args {
        // We accept simulation trace as either options or positional arguments
        let simulation_trace = short('t')
            .long("trace")
            .help("vcd/fst file with simulation trace")
            .argument::<String>("TRACE")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(fst|vcd)"),
            });
        let bus_description = short('b')
            .long("bus-config")
            .help("yaml with description of buses")
            .argument::<String>("BUS_CONFIG")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(yaml|yml)"),
            });
        let opt = construct!(FileArgs {
            simulation_trace,
            bus_description
        });
        let simulation_trace = positional("TRACE")
            .help("vcd/fst file with simulation trace")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(fst|vcd)"),
            });
        let bus_description = positional("BUS")
            .help("yaml with description of buses")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(yaml|yml)"),
            });
        let pos = construct!(FileArgs {
            simulation_trace,
            bus_description
        });
        let files = construct!([opt, pos]);

        let max_burst_delay = short('m')
            .long("max_burst_delay")
            .help("Max delay during a burst [default: 0]")
            .argument("BURST")
            .fallback(0);
        let output = short('o')
            .help("Output filename")
            .argument("OUT")
            .optional();

        let skipped_stats = short('s')
            .long("skip")
            .help("Stats to skip separated by a comma.")
            .argument::<String>("SKIPPED_STATS")
            .optional();

        let gui = long("gui").help("Run GUI").req_flag(OutputType::Rendered);
        let csv = long("csv")
            .help("Format output as csv")
            .req_flag(OutputType::Csv);
        let md = long("md")
            .help("Format output as md table")
            .req_flag(OutputType::Md);
        let text = long("text")
            .help("Format output as table")
            .req_flag(OutputType::Pretty);
        let output_type = construct!([gui, csv, md, text]);

        let window_length = short('w')
            .long("window")
            .help("Set size of the rolling window [default: 10000]")
            .argument("WINDOW")
            .fallback(10000);
        let x_rate = short('x')
            .long("x_rate")
            .help("Set x_rate for bandwidth above x_rate [default: 0.0001]")
            .argument("X_RATE")
            .fallback(0.0001);
        let y_rate = short('y')
            .long("y_rate")
            .help("Set y_rate for bandwidth below y_rate [default: 0.00001]")
            .argument("Y_RATE")
            .fallback(0.00001);
        let verbose = short('v').long("verbose").switch();

        let parser = construct!(Args {
            output_type,
            output,
            skipped_stats,
            max_burst_delay,
            window_length,
            x_rate,
            y_rate,
            verbose,
            files,
        });
        parser.run()
    }
}

fn main() {
    let args = Args::parse();
    let mut analyzers = load_bus_analyzers(
        &args.files.bus_description,
        args.max_burst_delay as i32,
        args.window_length,
        args.x_rate,
        args.y_rate,
    )
    .unwrap();
    let mut data = load_simulation_trace(&args.files.simulation_trace, args.verbose);
    for a in analyzers.iter_mut() {
        a.analyze(&mut data, args.verbose);
    }
    let mut out: &mut dyn std::io::Write = match args.output {
        None => &mut std::io::stdout(),
        Some(filename) => &mut std::fs::File::create(filename).unwrap(),
    };

    let skipped_stats_arg = args.skipped_stats.unwrap_or_default();
    let skipped_stats: Vec<String> = skipped_stats_arg
        .split(',')
        .map(|s| s.to_string())
        .collect();

    show_data(
        analyzers,
        args.output_type,
        Some(&mut out),
        &mut data,
        &args.files.simulation_trace,
        args.verbose,
        &skipped_stats,
    );
}
