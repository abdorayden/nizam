import json
from template import TEMPLATE

def init_nizam_template():
    with open("Nizam.json", "w") as fd:
        fd.write(TEMPLATE.dump())
