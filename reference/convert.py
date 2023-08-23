import os
import re
import shutil

r = re.compile(r'(.*?)_of_(.*?)2?.svg')

files = os.listdir(path='cards/')
files.sort(key=lambda f: '2' in f)

suits = {
    'clubs': 'C', 
    'spades': 'S', 
    'hearts': 'H', 
    'diamonds': 'D', 
}

ranks = {
    'ten': 'T', 
    'jack': 'J', 
    'queen': 'Q', 
    'king': 'K', 
    'ace': 'A', 
}

for file in files:
    s = r.search(file)
    if s is None:
        continue
    rank = s[1]
    rank = ranks[rank] if rank in ranks else rank
    suit = s[2]
    suit = suits[suit]

    shutil.copy2('cards/' + file, f'new/{rank}{suit}.svg')
    print(file)
    print(rank, suit)
    
