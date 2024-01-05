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

function checkWord(dictionary, board, tiles) {
    const word = [...tiles.querySelectorAll(`[value]`)]
        .sort((a, b) => +a.getAttribute('value') - +b.getAttribute('value'))
        .map(n => n.parentElement.childNodes[1].wholeText)
        .join('')
        .toLowerCase();
    board.querySelector('.spelling').textContent = word;
    const validWord = dictionary.has(word);
    tiles.classList.toggle('valid-word', validWord);
    if (validWord) {
        tiles.dispatchEvent(new CustomEvent('spell'));
    }
}

let dictionary = new Set();

fetch('words.txt')
    .then(d => d.text())
    .then(d => {
        dictionary = new Set(d.split('\n'));
    });

htmx.onLoad(setupBoard);

function setupBoard(root) {
    console.log('hello', root);
    const tiles = root.querySelector('.tiles');
    if (tiles === null) return;
    const board = root.querySelector('.board');

    let mouseDown = false;

    [...tiles.children].forEach(n => n.addEventListener('mousedown', e => {
        mouseDown = true;
        addTile(e.target, tiles, mouseDown, dictionary, 4);
        checkWord(dictionary, board, tiles);
    }));

    [...tiles.children].forEach(n => n.addEventListener('mouseenter', e => {
        addTile(e.target, tiles, mouseDown, dictionary, 4);
        checkWord(dictionary, board, tiles);
    }));

    document.addEventListener('mouseup', e => {
        mouseDown = false;
        tiles.querySelectorAll(`[value]`).forEach(n => n.removeAttribute('value'));
        checkWord(dictionary, board, tiles);
    });

}
