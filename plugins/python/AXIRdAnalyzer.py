class AXIRdAnalyzer:
    def __init__(self):
        print("Loaded AXIRAnalyzer")

    def get_yaml_signals(self):
        return ["ar_rdy", "ar_vld", "r_rdy", "r_vld"]

    def analyze(self, ar_rdy, ar_vld, r_rdy, r_vld):
        print(ar_rdy)
        # print(ar_rdy, ar_vld, r_rdy, r_vld)
        return [(0, 3, 2, 1, "01", 1), (5, 8, 7, 6, "10", 10)]

def create():
    return AXIRdAnalyzer()
