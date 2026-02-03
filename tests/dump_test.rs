use busperf::analyze::*;
use itertools::Itertools;
use libbusperf::bus_usage::{self, BusData, BusUsage, Period, SingleChannelBusUsage};

// helper function to check if analyzer returns expected result
fn test(trace: &str, yaml: &str, max_burst_delay: i32, correct: &[BusUsage]) {
    let mut data = load_simulation_trace(trace, false).unwrap();
    let descs = load_bus_analyzers(
        yaml,
        max_burst_delay,
        10000,
        0.0001,
        0.00001,
        "plugins/python",
    )
    .unwrap();
    let results = analyze_all(descs, &mut data, false);
    assert_eq!(correct.len(), results.len());
    for (r, correct) in results.iter().zip(correct) {
        assert_eq!(&r.as_ref().unwrap().usage, correct)
    }
}

// helper function to check if provided number of results has been calculated
fn test_basic(trace: &str, yaml: &str, num: usize) {
    let mut data = load_simulation_trace(trace, false).unwrap();
    let descs = load_bus_analyzers(yaml, 0, 10000, 0.0001, 0.00001, "plugins/python").unwrap();
    let results = analyze_all(descs, &mut data, false);
    assert_eq!(results.len(), num);
    for r in results {
        r.unwrap();
    }
}

// test dump.vcd - ready/valid with 2 iterfaces
#[test]
fn dump() {
    test(
        "tests/test_dumps/dump.vcd",
        "tests/test_dumps/dump.yaml",
        0,
        &[correct_dump_a(), correct_dump_b()],
    );
}

// test dump.vcd with reset type set to high
#[test]
fn dump_rst_high() {
    let correct_a = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "a_",
        15,
        0,
        0,
        0,
        0,
        15,
        vec![Period::literal(30000, 58000, 15)],
        vec![Period::literal(0, 28000, 15)],
        0,
        bus_usage::CurrentlyCalculating::Delay,
        2000,
    ));
    let correct_b = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "b_",
        0,
        0,
        15,
        0,
        0,
        15,
        vec![Period::literal(0, 58000, 30)],
        vec![],
        0,
        bus_usage::CurrentlyCalculating::Delay,
        2000,
    ));
    test(
        "tests/test_dumps/dump.vcd",
        "tests/test_dumps/dump_rst_high.yaml",
        0,
        &[correct_a, correct_b],
    );
}

// test test.vcd - ready/valid, one interface
#[test]
fn basic() {
    test(
        "tests/test_dumps/test.vcd",
        "tests/test_dumps/test.yaml",
        0,
        &[correct_test()],
    );
}

// test longer path to signals
#[test]
fn basic_scopes() {
    test_basic(
        "tests/test_dumps/test_complex_scope.vcd",
        "tests/test_dumps/test_complex_scope.yaml",
        2,
    );
}

#[test]
fn nested_scopes() {
    test_basic(
        "tests/test_dumps/nested_scopes.vcd",
        "tests/test_dumps/nested_scopes.yaml",
        2,
    );
}

#[test]
fn common_clk_rst_ifs() {
    test_basic(
        "tests/test_dumps/nested_scopes.vcd",
        "tests/test_dumps/clk_rst_if.yaml",
        2,
    );
}

// test whether max_burst_delay functions correctly
#[test]
fn basic_max_burst_delay() {
    let correct = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "test",
        9,
        5,
        3,
        0,
        3,
        2,
        vec![Period::literal(0, 2, 2), Period::literal(24, 34, 6)],
        vec![Period::literal(4, 22, 10), Period::literal(36, 42, 4)],
        2,
        bus_usage::CurrentlyCalculating::Burst,
        2,
    ));
    test(
        "tests/test_dumps/test.vcd",
        "tests/test_dumps/test.yaml",
        2,
        &[correct],
    );
}

// test for credit/valid bus
#[test]
fn credit_valid() {
    let correct = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "test",
        14,
        0,
        0,
        3,
        3,
        1,
        vec![
            Period::literal(0, 0, 1),
            Period::literal(10, 12, 2),
            Period::literal(16, 16, 1),
            Period::literal(22, 26, 3),
        ],
        vec![
            Period::literal(2, 8, 4),
            Period::literal(14, 14, 1),
            Period::literal(18, 20, 2),
            Period::literal(28, 40, 7),
        ],
        0,
        bus_usage::CurrentlyCalculating::Burst,
        2,
    ));
    test(
        "tests/test_dumps/credit_valid.vcd",
        "tests/test_dumps/credit_valid.yaml",
        0,
        &[correct],
    );
}

// test for ahb bus
#[test]
fn ahb() {
    let correct = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "test",
        9,
        5,
        1,
        0,
        5,
        1,
        vec![
            Period::literal(0, 0, 1),
            Period::literal(10, 12, 2),
            Period::literal(16, 16, 1),
            Period::literal(22, 32, 6),
            Period::literal(36, 38, 2),
        ],
        vec![
            Period::literal(2, 8, 4),
            Period::literal(14, 14, 1),
            Period::literal(18, 20, 2),
            Period::literal(34, 34, 1),
            Period::literal(40, 40, 1),
        ],
        0,
        bus_usage::CurrentlyCalculating::Burst,
        2,
    ));
    test(
        "tests/test_dumps/ahb.vcd",
        "tests/test_dumps/ahb.yaml",
        0,
        &[correct],
    );
}

// test apb bus
#[test]
fn apb() {
    let correct = BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "test",
        11,
        5,
        0,
        0,
        4,
        2,
        vec![
            Period::literal(0, 2, 2),
            Period::literal(12, 14, 2),
            Period::literal(18, 18, 1),
            Period::literal(30, 32, 2),
            Period::literal(36, 42, 4),
        ],
        vec![
            Period::literal(4, 10, 4),
            Period::literal(16, 16, 1),
            Period::literal(20, 28, 5),
            Period::literal(34, 34, 1),
        ],
        0,
        bus_usage::CurrentlyCalculating::Delay,
        2,
    ));
    test(
        "tests/test_dumps/apb.vcd",
        "tests/test_dumps/apb.yaml",
        0,
        &[correct],
    );
}

// test python ready/valid plugin on test.vcd
#[test]
fn python() {
    test(
        "tests/test_dumps/test.vcd",
        "tests/test_dumps/python_test.yaml",
        0,
        &[correct_test()],
    );
}

// test python ready/valid plugin on dump.vcd
#[test]
fn python_dump() {
    test(
        "tests/test_dumps/dump.vcd",
        "tests/test_dumps/python_dump.yaml",
        0,
        &[correct_dump_a(), correct_dump_b()],
    );
}

// test multichannel axi analyzer
#[test]
fn axi_test() {
    test_basic(
        "tests/test_dumps/axi.vcd",
        "tests/taxi_descriptions/axi_ram.yaml",
        2,
    );
}

#[test]
fn python_axi() {
    test_basic(
        "tests/test_dumps/axi.vcd",
        "tests/taxi_descriptions/axi_ram_python.yaml",
        1,
    );
}

#[test]
fn custom_plugin_path() {
    let mut data = load_simulation_trace("tests/test_dumps/test.vcd", false).unwrap();
    let descs = load_bus_analyzers(
        "tests/test_dumps/python_test.yaml",
        0,
        10000,
        0.0001,
        0.00001,
        "tests/dummy_plugins",
    )
    .unwrap();
    let results = analyze_all(descs, &mut data, false);
    assert!(results.len() == 1);
    for r in results {
        r.unwrap();
    }
}

#[test]
fn channel_and_signal_triggers() {
    let mut data = load_simulation_trace("tests/test_dumps/axi.vcd", false).unwrap();
    let descs = load_bus_analyzers(
        "tests/test_dumps/channel_and_signal_triggers.yaml",
        0,
        10000,
        0.0001,
        0.00001,
        "tests/dummy_plugins",
    )
    .unwrap();
    let results = analyze_all(descs, &mut data, false);
    let BusData {
        usage: BusUsage::MultiChannel(usage),
        ..
    } = results[0].as_ref().unwrap()
    else {
        panic!("invalid usage outputted")
    };
    let calculated = usage.get_transactions_start_end().iter().collect_vec();
    let ok = [
        &Period::literal(320000, 340000, 2),
        &Period::literal(1530000, 1550000, 2),
        &Period::literal(2820000, 2850000, 3),
        &Period::literal(4220000, 4250000, 3),
        &Period::literal(5690000, 5720000, 3),
        &Period::literal(7210000, 7240000, 3),
        &Period::literal(8790000, 8830000, 4),
    ];
    assert_eq!(calculated, ok);

    let BusData {
        usage: BusUsage::MultiChannel(usage),
        ..
    } = results[3].as_ref().unwrap()
    else {
        panic!("invalid usage outputted")
    };
    let calculated = usage.get_transactions_start_end().iter().collect_vec();
    let ok = [
        &Period::literal(560000, 580000, 2),
        &Period::literal(1790000, 1820000, 3),
        &Period::literal(3110000, 3140000, 3),
        &Period::literal(4510000, 4540000, 3),
        &Period::literal(5980000, 6010000, 3),
        &Period::literal(7520000, 7560000, 4),
        &Period::literal(9130000, 9170000, 4),
    ];
    assert_eq!(calculated, ok);
}

#[test]
fn transaction_triggers() {
    let mut data = load_simulation_trace("tests/test_dumps/axi.vcd", false).unwrap();
    let descs = load_bus_analyzers(
        "tests/test_dumps/transactions_triggers.yaml",
        0,
        10000,
        0.0001,
        0.00001,
        "tests/dummy_plugins",
    )
    .unwrap();
    let results = analyze_all(descs, &mut data, false);
    let BusData {
        usage: BusUsage::SingleChannel(usage),
        ..
    } = results[1].as_ref().unwrap()
    else {
        panic!("invalid usage outputted")
    };
    let calculated = usage
        .get_cycles()
        .data_labels
        .into_iter()
        .map(|(d, _)| d as u64)
        .collect_vec();
    let ok = [126, 0, 869, 0, 0, 0];
    assert_eq!(calculated, ok);
    let BusData {
        usage: BusUsage::SingleChannel(usage),
        ..
    } = results[3].as_ref().unwrap()
    else {
        panic!("invalid usage outputted")
    };
    let calculated = usage
        .get_cycles()
        .data_labels
        .into_iter()
        .map(|(d, _)| d as u64)
        .collect_vec();
    let ok = [308, 158, 0, 0, 539, 0];
    assert_eq!(calculated, ok);
}

#[test]
fn time_and_fsm_triggers() {
    let mut data = load_simulation_trace("tests/test_dumps/axi.vcd", false).unwrap();
    let descs = load_bus_analyzers(
        "tests/test_dumps/time_and_fsm_triggers.yaml",
        0,
        10000,
        0.0001,
        0.00001,
        "tests/dummy_plugins",
    )
    .unwrap();
    let results = analyze_all(descs, &mut data, false);
    let BusData {
        usage: BusUsage::MultiChannel(usage),
        ..
    } = results[0].as_ref().unwrap()
    else {
        panic!("invalid usage outputted")
    };
    let calculated = usage.get_transactions_start_end().iter().collect_vec();
    let ok = [
        &Period::literal(170000, 200000, 3),
        &Period::literal(320000, 340000, 2),
        &Period::literal(600000, 630000, 3),
        &Period::literal(760000, 790000, 3),
    ];
    assert_eq!(calculated, ok);
}

// functions returning correct usages for tests

fn correct_test() -> BusUsage {
    BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "test",
        9,
        5,
        3,
        0,
        3,
        2,
        vec![
            Period::literal(0, 2, 2),
            Period::literal(12, 14, 2),
            Period::literal(18, 18, 1),
            Period::literal(24, 34, 6),
            Period::literal(38, 40, 2),
        ],
        vec![
            Period::literal(4, 10, 4),
            Period::literal(16, 16, 1),
            Period::literal(20, 22, 2),
            Period::literal(36, 36, 1),
            Period::literal(42, 42, 1),
        ],
        0,
        bus_usage::CurrentlyCalculating::Burst,
        2,
    ))
}

fn correct_dump_a() -> BusUsage {
    BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "a_",
        0,
        0,
        15,
        0,
        0,
        15,
        vec![Period::literal(0, 58000, 30)],
        vec![],
        0,
        bus_usage::CurrentlyCalculating::Delay,
        2000,
    ))
}

fn correct_dump_b() -> BusUsage {
    BusUsage::SingleChannel(SingleChannelBusUsage::literal(
        "b_",
        0,
        0,
        15,
        0,
        0,
        15,
        vec![Period::literal(0, 58000, 30)],
        vec![],
        0,
        bus_usage::CurrentlyCalculating::Delay,
        2000,
    ))
}
