class AXIRdAnalyzer:
    def __init__(self):
        print("Loaded AXIRAnalyzer")

    def get_yaml_signals(self):
        return ["ar_rdy", "ar_vld", "r_rdy", "r_vld", "r_resp"]

    def analyze(self, clk, rst, ar_rdy, ar_vld, r_rdy, r_vld, r_resp):
        time_end = clk[-1][0]
        next_time = list(map(lambda r: r[0], ar_vld))
        next_time = next_time[2:]
        next_time.append(time_end)
        next_time.append(time_end)
        r_resp.reverse()
        transactions = []
        
        for ((time, value), next_time) in zip(ar_vld, next_time):
            if value != "1":
                continue
            first_data = next(filter(lambda r: r[0] > time and r[1] == "1",  r_vld))
            first_data = first_data[0]
            resp_time = first_data
            last_data = first_data
            delay = next_time - resp_time
            resp = next(filter(lambda r: r[0] <= resp_time, r_resp))
            print(resp)

            transactions.append((time, resp_time, last_data, first_data, resp[1], delay))
        return transactions

def create():
    return AXIRdAnalyzer()
