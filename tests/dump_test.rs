use busperf::*;

fn test(trace: &str, yaml: &str, max_burst_delay: u32, correct: &[BusUsage]) {
    let mut data = load_simulation_trace(trace);
    let descs = load_bus_descriptions(yaml, max_burst_delay).unwrap();
    assert_eq!(correct.len(), descs.len());
    for (desc, correct) in descs.iter().zip(correct) {
        let usage = calculate_usage(&mut data, &**desc);
        assert_eq!(usage, *correct);
    }
}

#[test]
fn dump() {
    let correct_a = BusUsage::literal(
        "a_",
        0,
        0,
        15,
        15,
        0,
        vec![30],
        0,
        vec![0, 0, 0, 0, 1],
        vec![],
        vec![],
        0,
        0,
        0,
    );
    let correct_b = BusUsage::literal(
        "b_",
        0,
        0,
        15,
        15,
        0,
        vec![30],
        0,
        vec![0, 0, 0, 0, 1],
        vec![],
        vec![],
        0,
        0,
        0,
    );
    test(
        "tests/test_dumps/dump.vcd",
        "tests/test_dumps/dump.yaml",
        0,
        &[correct_a, correct_b],
    );
}

#[test]
fn dump_rst_high() {
    let correct_a = BusUsage::literal(
        "a_",
        15,
        0,
        0,
        15,
        0,
        vec![15],
        0,
        vec![0, 0, 0, 1],
        vec![15],
        vec![0, 0, 0, 1],
        0,
        1,
        0,
    );
    let correct_b = BusUsage::literal(
        "b_",
        0,
        0,
        15,
        15,
        0,
        vec![30],
        0,
        vec![0, 0, 0, 0, 1],
        vec![],
        vec![],
        0,
        0,
        0,
    );
    test(
        "tests/test_dumps/dump.vcd",
        "tests/test_dumps/dump_rst_high.yaml",
        0,
        &[correct_a, correct_b],
    );
}

#[test]
fn basic() {
    let correct = BusUsage::literal(
        "test",
        9,
        5,
        3,
        1,
        3,
        vec![1, 2, 1, 6, 2],
        5,
        vec![2, 2, 1],
        vec![4, 1, 2, 1, 1],
        vec![3, 1, 1],
        0,
        4,
        0,
    );
    test(
        "tests/test_dumps/test.vcd",
        "tests/test_dumps/test.yaml",
        0,
        &[correct],
    );
}

#[test]
fn basic_scopes() {
    let correct = BusUsage::literal(
        "test",
        9,
        5,
        3,
        1,
        3,
        vec![1, 2, 1, 6, 2],
        5,
        vec![2, 2, 1],
        vec![4, 1, 2, 1, 1],
        vec![3, 1, 1],
        0,
        4,
        0,
    );
    test(
        "tests/test_dumps/test_complex_scope.vcd",
        "tests/test_dumps/test_complex_scope.yaml",
        0,
        &[correct],
    );
}

#[test]
fn basic_max_burst_delay() {
    let correct = BusUsage::literal(
        "test",
        9,
        5,
        3,
        1,
        3,
        vec![6],
        1,
        vec![0, 0, 1],
        vec![11, 4],
        vec![0, 0, 1, 1],
        6,
        1,
        2,
    );
    test(
        "tests/test_dumps/test_complex_scope.vcd",
        "tests/test_dumps/test_complex_scope.yaml",
        2,
        &[correct],
    );
}

#[test]
fn credit_valid() {
    let correct = BusUsage::literal(
        "test",
        14,
        0,
        0,
        5,
        2,
        vec![1, 2, 1, 3],
        4,
        vec![2, 2],
        vec![4, 1, 2, 7],
        vec![1, 1, 2],
        0,
        3,
        0,
    );
    test(
        "tests/test_dumps/credit_valid.vcd",
        "tests/test_dumps/credit_valid.yaml",
        0,
        &[correct],
    );
}

#[test]
fn ahb() {
    let correct = BusUsage::literal(
        "test",
        9,
        5,
        1,
        1,
        5,
        vec![1, 2, 1, 6, 2],
        5,
        vec![2, 2, 1],
        vec![4, 1, 2, 1, 1],
        vec![3, 1, 1],
        0,
        4,
        0,
    );
    test(
        "tests/test_dumps/ahb.vcd",
        "tests/test_dumps/ahb.yaml",
        0,
        &[correct],
    );
}
