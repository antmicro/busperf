use bpaf::{construct, long, positional, short, OptionParser, Parser};
use busperf::show::OutputType;
use cfg_if::cfg_if;
use owo_colors::OwoColorize;

enum Args {
    Analyze(AnalyzeArgs),
    Show(ShowArgs),
}

impl Args {
    fn parse() -> Args {
        let analyze = AnalyzeArgs::parse()
            .to_options()
            .descr("Analyze given trace")
            .command("analyze");
        let show = ShowArgs::parse()
            .to_options()
            .descr("Show statistics from a file")
            .command("show");

        let parser: OptionParser<Args> = construct!([analyze, show]).to_options();
        let mut args = parser.run();

        // swap simulation trace and bus description when files are passed in wrong order
        if let Args::Analyze(args) = &mut args {
            let files = &mut args.files;
            if files.simulation_trace.ends_with(".yaml")
                && (files.bus_description.ends_with(".fst")
                    || files.bus_description.ends_with(".vcd"))
            {
                std::mem::swap(&mut files.simulation_trace, &mut files.bus_description);
            }
        }
        args
    }
}

struct ShowArgs {
    file: String,
    output_type: OutputType,
    verbose: bool,
}

impl ShowArgs {
    pub fn parse() -> impl Parser<Args> {
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
        let verbose = short('v').long("verbose").switch();
        let file = positional("FILENAME").help("File to load statistics from");

        let parser = construct!(ShowArgs {
            output_type,
            verbose,
            file,
        });
        construct!(Args::Show(parser))
    }
}

struct AnalyzeArgs {
    files: FileArgs,
    max_burst_delay: u32,
    verbose: bool,
    output: Option<String>,
    skipped_stats: Option<String>,
    output_type: OutputType,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
    plugins_path: String,
}

struct FileArgs {
    simulation_trace: String,
    bus_description: String,
}

impl AnalyzeArgs {
    pub fn parse() -> impl Parser<Args> {
        // We accept simulation trace as either options or positional arguments
        let simulation_trace = positional("TRACE")
            .help("vcd/fst file with simulation trace")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(fst|vcd)"),
            });
        let bus_description = positional("BUS_CONFIG")
            .help("yaml with description of buses")
            .complete_shell(bpaf::ShellComp::File {
                mask: Some("*.(yaml|yml)"),
            });
        let files = construct!(FileArgs {
            simulation_trace,
            bus_description
        });

        let max_burst_delay = short('m')
            .long("max_burst_delay")
            .help("Max delay during a burst [default: 0]")
            .argument("BURST")
            .fallback(0);
        let output = short('o')
            .long("output")
            .help("Output filename")
            .argument("OUT")
            .optional();

        let skipped_stats = long("skip")
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

        let data = long("save")
            .help("Save data in busperf format (requires setting -o)")
            .req_flag(OutputType::Data);
        cfg_if! {
            if #[cfg(feature = "generate-html")] {
                let html = long("html")
                    .help("Generate HTML with embedded busperf_web (requires setting -o)")
                    .req_flag(OutputType::Html);
                let output_type = construct!([gui, csv, md, text, data, html]);
            } else {
                let output_type = construct!([gui, csv, md, text, data]);
            }
        }

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
        let plugins_path = short('p')
            .long("plugins_path")
            .help("Path to python plugins [default: \"./plugins/python]\"")
            .argument("PATH")
            .fallback("./plugins/python".to_string());

        let parser = construct!(AnalyzeArgs {
            output_type,
            output,
            skipped_stats,
            max_burst_delay,
            window_length,
            x_rate,
            y_rate,
            verbose,
            plugins_path,
            files,
        });
        construct!(Args::Analyze(parser))
    }
}

fn main() {
    let args = Args::parse();
    match args {
        Args::Analyze(args) => {
            let skipped_stats_arg = args.skipped_stats.unwrap_or_default();
            let skipped_stats: Vec<String> = skipped_stats_arg
                .split(',')
                .map(|s| s.to_string())
                .collect();
            use busperf::{
                analyze::{load_bus_analyzers, load_simulation_trace},
                run_visualization,
            };

            let analyzers = match load_bus_analyzers(
                &args.files.bus_description,
                args.max_burst_delay as i32,
                args.window_length,
                args.x_rate,
                args.y_rate,
                &args.plugins_path,
            ) {
                Ok(analyzers) => analyzers,
                Err(e) => {
                    eprintln!(
                        "{} {}",
                        "[ERROR] Invalid bus decription:".bright_red(),
                        e.bright_red()
                    );
                    std::process::exit(1);
                }
            };

            let mut data = load_simulation_trace(&args.files.simulation_trace, args.verbose)
                .unwrap_or_else(|e| {
                    eprintln!(
                        "{} {}",
                        "[ERROR] Invalid simulation trace:".bright_red(),
                        e.bright_red()
                    );
                    std::process::exit(1);
                });
            if let OutputType::Data = args.output_type {
                args.output.as_ref().unwrap_or_else(|| {
                    eprintln!(
                        "Error: Output file name (-o option) is required for data output type."
                    );
                    std::process::exit(1)
                });
            }
            #[cfg(feature = "generate-html")]
            if let OutputType::Html = args.output_type {
                args.output.as_ref().unwrap_or_else(|| {
                    eprintln!(
                        "Error: Output file name (-o option) is required for html output type."
                    );
                    std::process::exit(1)
                });
            }

            let mut out: &mut dyn std::io::Write = match args.output.clone() {
                None => &mut std::io::stdout(),
                Some(filename) => &mut std::fs::File::create(filename).unwrap_or_else(|e| {
                    eprintln!(
                        "{} {}",
                        "[ERROR] Failed to create output file:".bright_red(),
                        e.bright_red()
                    );
                    std::process::exit(1);
                }),
            };
            if let Err(e) = run_visualization(
                analyzers,
                args.output_type,
                &mut out,
                &mut data,
                args.files.simulation_trace,
                args.verbose,
                &skipped_stats,
            ) {
                eprintln!("{} {}", "[ERROR]".bright_red(), e.bright_red());
                std::process::exit(1);
            }
        }
        Args::Show(args) => {
            use busperf::show::visualization_from_file;

            if let Err(e) = visualization_from_file(&args.file, args.output_type, args.verbose) {
                eprintln!("{} {}", "[ERROR]".bright_red(), e.bright_red());
                std::process::exit(1);
            }
        }
    }
}
