from busperf import CycleType

class Analyzer:
    def get_signals(self):
        return ["ready", "valid"]
    def interpret_cycle(self, signals):
        return CycleType.Unknown
    def provides(self):
        return ["trig1", "trig2"]
    def get_trigger(self, signals):
        triggers = []
        if signals[0] == "1":
            triggers.append("trig1")
        if signals[1] == "1":
            triggers.append("trig2")
        return triggers

def create():
    return Analyzer()
