from collections import defaultdict, deque
from more_itertools import peekable
from busperf import SignalType
from busperf import Transaction


class InProgressTransaction:
    def __init__(self, start, next_time):
        self.start = start
        self.next = next_time
        self.first_data = None


class Analyzer:
    def __init__(self):
        print("Loaded AXIFullRdAnalyzerPython")

    def get_yaml_signals(self):
        return [(SignalType.ReadyValid, ["ar"]),
                (SignalType.ReadyValid, ["r"]),
                (SignalType.Signal, ["r", "rresp"]),
                (SignalType.Signal, ["ar", "id"]),
                (SignalType.Signal, ["r", "id"]),
                (SignalType.Signal, ["r", "rlast"])]

    def analyze(self, clk, rst, ar, r, r_resp, ar_id, r_id, r_last):
        time_end = clk[-1][0]
        to_return = []

        counting = defaultdict(deque)
        unfinished = ""

        ar = peekable(iter(map(lambda v: v[0], ar)))
        r = peekable(iter(map(lambda v: v[0], r)))
        rst = iter(map(lambda v: v[0], rst))
        ar_id.reverse()
        r_last.reverse()
        r_id.reverse()
        next_rst = next(rst, time_end + 1)

        while True:
            try:
                time = next(ar)
            except StopIteration:
                break

            while next_rst < time:
                next_rst = next(rst, time_end + 1)

            ar_id_value = next(filter(lambda v: v[0] < time, ar_id))[1]
            next_transaction = ar.peek(time_end)

            counting[ar_id_value].append(InProgressTransaction(time, next_transaction))

            while r.peek(next_transaction) < next_transaction:
                read = r.peek()

                if read > next_rst:
                    all_times = [
                        t.start
                        for txns in counting.values()
                        for t in txns
                    ]
                    unfinished += ", ".join(str(t) for t in all_times)
                    counting.clear()
                    break
                next(r)

                id_value = next(filter(lambda v: v[0] < read, r_id))[1]
                transactions = counting.get(id_value)
                assert transactions, f"Id {id_value} should be valid {read}"
                t = transactions[0]

                if t.first_data is None:
                    t.first_data = read

                resp = next(filter(lambda v: v[0] < read, r_resp), None)[1]

                if next(filter(lambda v: v[0] < read, r_last))[1] == "1":
                    completed = transactions.popleft()
                    to_return.append(Transaction(
                        completed.start,
                        completed.first_data,
                        read,
                        read,
                        resp,
                        completed.next,
                    ))

        leftover_times = [
            t.start
            for txns in counting.values()
            for t in txns
        ]
        if leftover_times:
            unfinished += ", " + ", ".join(str(t) for t in leftover_times)

        if unfinished.strip():
            print(f"[WARN] Unfinished transactions at times: {unfinished.strip(', ')}")
        return to_return


def create():
    return Analyzer()
