# nizam module is rust engine


"""

    ones you run the python script will check for Nizam.json template file to start build or run tasks

    nizam subcommands:
        init - will create Nizam.json template file fully documented

EXAMPLE: 
    $ nizam 


""" 

import nizam
from init import init_nizam_template

if __name__ == "__main__":
    init_nizam_template()
    print(nizam.add(1,2))
