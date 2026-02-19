
class Analyzer:
    def get_yaml_signals(self):
        return []

    def analyze(self, clk, rst):
        # times at which each trigger activates should be determined in analyze
        # method becuase here there is an access to signals
        self.trig1 = [0, 20, 31]
        self.trig2 = [10, 25, 34]
        return []

    def provides(self):
        return ["trig1", "trig2"]

    def get_trigger_times(self):
        # here only return the times calculated in analyze
        return {
            "trig1": self.trig1,
            "trig2": self.trig2,
        }


def create():
    return Analyzer()
