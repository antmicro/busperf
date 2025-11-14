from busperf import CycleType

class Analyzer:
    def __init__(self):
        print("Loaded python PythonReadyValid")
    def get_signals(self):
        return ["ready", "valid"]
    def interpret_cycle(self, signals):
        if signals[0] == "1" and signals[1] == "1":
            return CycleType.Busy
        if signals[0] == "0" and signals[1] == "0":
            return CycleType.Free
        if signals[0] == "0" and signals[1] == "1":
            return CycleType.Backpressure
        if signals[0] == "1" and signals[1] == "0":
            return CycleType.NoData
        return CycleType.Unknown
            

def create():
    return Analyzer()
