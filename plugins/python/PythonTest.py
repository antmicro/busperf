class Analyzer:
    def __init__(self):
        print("Loaded python Analyzer")
    def get_signals(self):
        return ["ready", "valid"]
    def interpret_cycle(self, signals):
        if signals[0] == "1" and signals[1] == "1":
            return 0
        if signals[0] == "0" and signals[1] == "0":
            return 1
        if signals[0] == "0" and signals[1] == "1":
            return 3
        if signals[0] == "1" and signals[1] == "0":
            return 4
        # print(signals[0], signals[1])
        return 2
            

def create():
    return Analyzer()
