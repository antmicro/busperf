from more_itertools import peekable
from busperf import Transaction
from busperf import SignalType


class Analyzer:
    def __init__(self):
        print("Loaded AXILiteRdAnalyzerPython")

    def get_yaml_signals(self):
        return [(SignalType.ReadyValid, ["ar"]),
                (SignalType.ReadyValid, ["r"]),
                (SignalType.Signal, ["r", "rresp"])]

    def analyze(self, clk, rst, ar, r, rresp):
        time_end = clk[-1][0]
        rst = peekable(iter(map(lambda v: v[0], rst)))
        ar = peekable(iter(map(lambda v: v[0], ar)))
        r = peekable(iter(map(lambda v: v[0], r)))
        rresp.reverse()
        transactions = []

        next_rst = next(rst, time_end + 1)

        while True:
            try:
                time = next(ar)
            except StopIteration:
                break

            while next_rst < time:
                next_rst = next(rst, time_end + 1)

            if r.peek() is not None and next_rst > r.peek():
                read_time = r.peek()

                next_transaction = ar.peek(time_end)

                next(r)

                while r.peek(next_transaction) < next_transaction:
                    print(f"[WARN] Read without AR at {r.peek()}")
                    next(r)

                resp = next(filter(lambda v: v[0] < read_time, rresp), None)[1]

                transactions.append(Transaction(
                    time,
                    read_time,
                    read_time,
                    read_time,
                    resp,
                    next_transaction
                ))
            else:
                print("[WARN] unfinished transaction")

        return transactions


def create():
    return Analyzer()
