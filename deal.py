import numpy as np


def deal(card_count, hands_count):
    # tf.random.shuffle(tf.range(card_count), dtype=tf.int32)
    cards = np.random.default_rng().permutation(card_count) 
    cards = np.reshape(cards, (hands_count, -1))
    rows = np.reshape(np.arange(hands_count), (hands_count, -1))
    ret = np.full((hands_count, card_count), False)
    ret[rows, cards] = True
    return ret


e = deal(52, 4)

print(np.sum(e, axis=0))
print(np.sum(e, axis=1))
print(e)
#d = deal(52)
#
#print(np.sum(d, axis=0, keepdims=True))
#
#print(random_booleans(10))
