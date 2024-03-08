'use strict';

function addTile(tile, tiles, mouseDown, dictionary, columns) {
    if (!mouseDown) return;

    const selected = [...tiles.querySelectorAll(`[value]`)];

    if (tile.firstChild.hasAttribute('value')) {
        const order = +tile.firstChild.getAttribute('value');
        selected
            .filter(n => +n.getAttribute('value') > order)
            .forEach(n => n.removeAttribute('value'));
        return;
    }

    const lastOrder = Math.max(...selected.map(n => +n.getAttribute('value')));
    if (!isFinite(lastOrder)) {
        tile.firstChild.setAttribute('value', 0);
        tiles.classList.remove('valid-word');
        return;
    }

    const a = +tiles
        .querySelector(`label > input[value="${lastOrder}"]`)
        .getAttribute('name');
    const b = +tile.firstChild.getAttribute('name');
    const dr = Math.floor(a / columns) - Math.floor(b / columns);
    const dc = a % columns - b % columns;
    const adjacent = a !== b && Math.max(Math.abs(dr), Math.abs(dc)) <= 1;
    if (!adjacent) return;

    tile.firstChild.setAttribute('value', 1 + lastOrder);
}

function checkWord(dictionary, alreadySpelled, spelling, tiles) {
    const word = [...tiles.querySelectorAll(`[value]`)]
        .sort((a, b) => a.getAttribute('value') - b.getAttribute('value'))
        .map(n => n.parentElement.childNodes[1].wholeText)
        .join('');
    spelling.textContent = word;
    const validWord = dictionary.has(word);
    tiles.classList.toggle('valid-word', validWord);
    tiles.classList.toggle('already-spelled', alreadySpelled.has(word));
    if (validWord && !alreadySpelled.has(word)) {
        tiles.dispatchEvent(new CustomEvent('spell'));
        alreadySpelled.add(word);
    }
}

async function setupBoard(root) {
    console.log(root);
    const tiles = root.querySelector('.tiles');
    const spelling = root.querySelector('.spelling');
    if (tiles === null || spelling === null) return;
    if (root.getAttribute('data-initialized')) return;
    root.setAttribute('data-initialized', true); 

    let dictionary;
    dictionary = await fetch('words.txt');
    dictionary = await dictionary.text();
    dictionary = new Set(dictionary.split('\n'));

    const alreadySpelled = new Set();

    let mouseDown = false;

    [...tiles.children].forEach(n => n.addEventListener('mousedown', e => {
        mouseDown = true;
        addTile(e.target, tiles, mouseDown, dictionary, 4);
        checkWord(dictionary, alreadySpelled, spelling, tiles);
    }));

    [...tiles.children].forEach(n => n.addEventListener('mouseenter', e => {
        addTile(e.target, tiles, mouseDown, dictionary, 4);
        checkWord(dictionary, alreadySpelled, spelling, tiles);
    }));

    document.addEventListener('mouseup', e => {
        mouseDown = false;
        tiles.querySelectorAll(`[value]`).forEach(n => n.removeAttribute('value'));
        checkWord(dictionary, alreadySpelled, spelling, tiles);
    });

}
