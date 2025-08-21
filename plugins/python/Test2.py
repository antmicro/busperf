class Analyzer2:
    def __init__(self):
        print("Loaded python Analyzer2")
    def get_signals(self):
        return ["ready", "valid"]
    def interpret_cycle(self, signals):
        return 0
            

def create():
    return Analyzer2()
