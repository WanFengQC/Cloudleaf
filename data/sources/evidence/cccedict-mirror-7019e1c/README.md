The data for CC-CEDICT and CC-Canto, preprocessed into keyed hashmap JSONs for programmatic convenience, along with a Python library to provide access to that data.

Looking online, I've only found parsers in varying languages for the data of CC-EDICT (and none for CC-Canto) , but not the data itself preprocessed, which can be annoying to deal with as you have to figure out how to install it, so I created this project to provide that pre-processed data, as a variety of keyed hashmaps. As well, spawning off this project is also a Python library for generating and updating these JSONs, along with accessing the data, although that second part is probably a dime a dozen.

Each JSON file is affixed with "key_[something]" which denotes which field the hashmap uses as the key. The valid key fields are simplified, traditional, pinyin, jyutping (for CC-Canto), definition (a python list saved as a string), and none, which is a simple list of the dictionary entries.

Provided as well are the raw zips for the data and the text files, dated to allow you to check for recency. Maybe someday I'll get it to update monthly, if I can figure out how to get Github Actions to work for that.

## The Python Library

https://pypi.org/project/py-cc-dicts/

### Installation

```
pip install py-cc-dicts
```

### Modules

The core Python library consists of three files: _parser.py_, which handles parsing the raw text files sourced from the CC-CEDICT and CC-Canto websites and creating the JSONs; _update.py_, which handles fetching the data from those websites and calls functions from parser to generate the JSONS in the right place; and _CC\_Dict.py_, which provides the the class CC_Dict for easier programmatic access of the paths for the JSONs or the data in the JSONs.

The two modules you'll most likely work with are _update.py_ and _CC\_Dict.py_.

### _CC\_Dict.py_

#### Core Class

```
from py_cc_dicts.CC_Dict import *

c = CC_Dict("CANTO") # Creates a CC_Dict object that can access the JSONs and dictionary data for CC-Canto. 
m = CC_Dict("CEDICT") # Creates a CC_Dict object that can access the JSONs and dictionary data for CC-CEDICT.
r = CC_Dict("READINGS") # Creates a CC_Dict object that can access the JSONs and readings data for the jyutping readings of CC-CEDICT as provided on the CC-Canto website.

# Loads the data from the dictionary website if not already existing into the current directory.

dicts = [CC_Dict("canto"), CC_Dict("cedict"), CC_Dict("readings")]
# Not case sensitive, the above works as well.
```

```
c.get_data(key = None) 
m.get_data(key = None)
# Get the dictionary data keyed with input *key* as a dict

c = CC_Dict("CANTO", data_dir = "some dir") # Creates a CC_Dict and stores the loaded data from the website at *data_dir* if it already does not exist in *data_dir*

c = CC_Dict("CANTO", update = True)
m = CC_Dict("CEDICT", data_dir = "some dir", update = True)
# Forcefully update the data by downloading it from the website and regenerating the JSONs, even if they already exists in either the current directory if none entred, or at *data_dir*
```

```
c2 = CC_Dict("CANTO", key = "traditional")
# By default load the dictionary data keyed by the input key into the CC_Dict's internal dict

c2.dict # Produces the dict keyed by traditional

# You can also search with dict syntax.
c2["出發"]
# Produces:
{'traditional': '出發', 'simplified': '出发', 'pinyin': 'chu1 fa1', 'jyutping': 'ceot1 faat3', 'definitions': ['to depart']}

c2["貓"]
# Produces (since there are multiple entries for the same key, they're provided as a list):
[{'traditional': '貓', 'simplified': '猫', 'pinyin': 'mao1', 'jyutping': 'maau1', 'definitions': ['cat M: 只zhī [只]', '(dialect) to hide oneself', '(coll.) modem', "to arch one's back", 'to be drunk', 'to be high on drugs']}, 
{'traditional': '貓', 'simplified': '猫', 'pinyin': 'mao1', 'jyutping': 'maau4', 'definitions': ['cat M: 只zhī [只]', '(dialect) to hide oneself', '(coll.) modem', "to arch one's back", 'to be drunk', 'to be high on drugs']}, 
{'traditional': '貓', 'simplified': '猫', 'pinyin': 'mao1', 'jyutping': 'miu4', 'definitions': ['cat M: 只zhī [只]', '(dialect) to hide oneself', '(coll.) modem', "to arch one's back", 'to be drunk', 'to be high on drugs']}]

c2.keys()
c2.values()
c2.items()
# As CC_Dict is an extension of dict, common dict functions also work, although some might have unintended behaviour if key = "definitions" (see below)
```

```
c3 = CC_Dict("CANTO", key = "definitions")
# If the key given is "definitions", allows for the search of all definitions via dict syntax.

c3["some string"]
# This would search and return all definitions for at contain the exact substring "some string" (as definitions are stored as strings)
```

### _update.py_

#### Core Functions

```
from py_cc_dicts.update import *

load_latest_data() # Load to current working directory
load_latest_data("*insert path here*") # Load to provided path

# Load the raws, the plain txt files and the JSONS for both CC-CEDICT and CC-Canto to input directory, if provided, else to current working directory.
```

```
fetch_raw() 

# Loads the zip files from the CC-CEDICT and CC-CANTO website to the *current working directory*
```

```
generate_jsons("path to zip directory")

# Takes the path to the directory where the raw data is stored and outputs the parsed JSONs for each key type to the *current working directory*
```

```
get_jsons(dir = "", dict_type = "")
get_raws(dir = "", dict_type = "")

# Search dir for jsons or raw zip files of the input dict_type (CEDICT, CANTO), or both if no dict_type is provided, and returns a list of strings containing the paths to those files.
```

```
jsons_exists(dir = "")
raws_exists(dir = "")

# Check if the jsons or raw zip files exist in directory *dir*, or the current working directory if none provided.
```

```
clean_raws(dir = "")
clean_jsons(dir = "")

# Delete the raw zip files or JSONs from directory *dir*, or the current working directory if none provided.

```

### _parser.py_

#### Constants

```
from py_cc_dicts.parser import *

DICT_TYPES = ["CEDICT", "CANTO"] # Valid Dictionary Codes, used throughout the program.
VALID_KEYS = {DICT_TYPES[0]: ["traditional", "simplified", "pinyin", "definitions", None],
               DICT_TYPES[1]: ["traditional", "simplified", "pinyin", "jyutping", "definitions", None]}  # Valid keys for CC_Dict, used for creation of JSONs
```

#### Core Functions

```
parse_cc_canto(filepath, key = "traditional")
parse_cc_cedict(filepath, key = "traditional", surnames = True)

# Parse the respective raw text file at *filepath* to produce a JSON with the given *key*. Surnames is currently unused.
```

## Changelog

### V 1.1
Can now access the jyupting readings data for CC-CEDICT as provided on the CC-Canto website.

```
r = CC_Dict("READINGS", key = "traditional")
r["試驗"]

# Returns:
{'traditional': '試驗', 'simplified': '试验', 'pinyin': 'shi4 yan4'}
```
