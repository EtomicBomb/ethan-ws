'use strict';
const $ = document.querySelector.bind(document);

//type Seat = 'north' | 'east' | 'south' | 'west';
const SEATS = ['north', 'east', 'south', 'west'];
const relatives = ['my', 'left', 'across', 'right'];

let game;

window.addEventListener('load', async (e) => {
    const sessionId = new URL(document.location).searchParams.get('sessionId');
    try {
        game = await Game.create(sessionId);
    } catch (e) {
        console.error('failed to create game with provided sessionId', e);
        game = await Game.create(null);
    }
    await game.eventLoop();
});

class Game {
    cards = null;

    auth = null;
    items = null;
    seatToRelative = null;
    progressBars = {};

    static async create(sessionId) {
        const url = new URL('/api/join', document.location);
        if (sessionId !== null) {
            url.searchParams.set('sessionId', sessionId);
        }
        const method = 'POST';
        const headers = { 'Accept': 'application/json-seq' };
        const response = await fetch(url, { method, headers });
        if (!response.ok) {
            throw new Error('failed to join', { cause: await response.text() });
        }
        const auth = JSON.parse(atob(response.headers.get('Authorization').split(' ')[1]));
        const items = response.body
            .pipeThrough(new TextDecoderStream())
            .pipeThrough(new TransformStream(new JsonSeqStream()))
            .getReader();
        return new Game(auth, items);
    }

    constructor(auth, items) {
        this.auth = auth;
        this.items = items;

        const sharableURL = new URL(document.location);
        sharableURL.searchParams.set('sessionId', auth.sessionId);
        window.history.pushState({}, '', sharableURL);

        const offset = SEATS.indexOf(this.auth.seat);
        this.seatToRelative = {};
        for (let i = 0; i<SEATS.length; i++) {
            this.seatToRelative[SEATS[i]] = relatives[(i-offset+4)%relatives.length];
        }

        console.log(this.seatToRelative);

        for (const [seat, relative] of Object.entries(this.seatToRelative)) {
            $(`.${relative} .name`).textContent = seat;
        }
        
        for (const relative of relatives) {
            this.progressBars[relative] = new ProgressBar($(`.${relative} .timer`));
        }
    }

    async eventLoop() {
        const dispatch = {
            retry: this.onRetry.bind(this),
            welcome: this.onWelcome.bind(this),
            host: this.onHost.bind(this),
            connected: this.onConnected.bind(this),
            deal: this.onDeal.bind(this),
            play: this.onPlay.bind(this),
            turn: this.onTurn.bind(this),
            win: this.onWin.bind(this),
            disconnected: this.onDisconnected.bind(this),
        };

        for (;;) {
            const { value, done } = await this.items.read();
            if (done) break;
            const { event, data } = value;
            console.log('received', event, data);
            await dispatch[event](data);
        }
    }

    async makeRequest(endpoint, method, body) {
        // FIXME: using the first argument of URL is wrong, because
        // we want to be robust to hosting the game at like etomicbomb.com/pusoy/ or pusoygame.com/
        const url = new URL(endpoint, document.location);
        const token = btoa(JSON.stringify(this.auth));
        const headers = { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' };
        body = JSON.stringify(body);
        return await fetch(url, { headers, method, body });
    }

    async onRetry({ error }) {
    }

    async onWelcome({ seat }) {

        await this.updateActionTimer();
        $('#enable-timer').addEventListener('change', this.updateActionTimer.bind(this));
        $('#set-timer').addEventListener('change', this.updateActionTimer.bind(this));
        $('#set-timer').addEventListener('input', async (e) => {
            const millis = parseInt(e.target.value, 10);
            $('#timer-value').textContent = `${Math.round(millis/1000)}s`;  
        });

        $('.my .start').addEventListener('click', async (e) => {
            const response = await this.makeRequest('/api/start', 'POST', { });
            if (!response.ok) {
                $('#error-description').textContent = await response.text();
            }
        });

        $('.my .play').addEventListener('click', async (e) => {
            const cards = [...document.querySelectorAll('.my .cards .card :checked')]
                .map(element => element.parentElement.dataset.card);
            const response = await this.makeRequest('/api/play', 'POST', { cards });
            if (!response.ok) {
                $('#error-description').textContent = await response.text();
            }
        });

    }

    async onHost({ }) {
        document.querySelectorAll('.host-config').forEach(elem => elem.style.display = 'unset');
    }

    async onConnected({ seat }) {
        const relative = this.seatToRelative[seat];
        $(`.${relative} .bot`).textContent = 'human';
    }

    async onDeal({ cards }) {
        document.querySelectorAll('.host-config').forEach(elem => elem.style.display = 'none');
        $('.my .start').style.display = 'none';
        $('.my .play').style.display = 'block';

        this.cards = cards;
        for (const relative of relatives) {
            const cardElements = relative === 'my'  
                ? this.cards.map(card => createCard(card, this))  
                : Array(13).fill(null).map(createBlank);
            $(`.${relative} .cards`).replaceChildren(...cardElements);
        }
        $(`.my .load`).textContent = this.cards.length;
    }

    async onPlay({ seat, load, cards, pass }) {
        const relative = this.seatToRelative[seat];
        this.progressBars[relative].stop();
        $(`.${relative} .load`).textContent = load;
        $(`.${relative} .turn`).textContent = '';
        $(`.${relative} .control`).textContent = '';
        if (relative === 'my') {
            this.updatePlayable();
            this.cards = this.cards.filter(card => !cards.includes(card));
        }

        if (pass) {
            $(`.${relative} .passed`).textContent = 'passed';
        } else {
            const cardElements = $(`.${relative} .cards`).children;
            const cardsToMove = relative === 'my'
                ? [...cardElements].filter(element => cards.includes(element.dataset.card))
                : chooseRandom([...cardElements], cards.length);
            // TODO: view transition
            for (let i=0; i<cards.length; i++) {
                const element = cardsToMove[i];
                const card = cards[i];
                element.remove();
                element.dataset.card = card;
            }
            $(`.table .cards`).replaceChildren(...cardsToMove);
            $(`.${relative} .load`).textContent = cardElements.length;
        }
    }

    async onTurn({ seat, millis, control }) {
        const relative = this.seatToRelative[seat];
        $(`.${relative} .turn`).textContent = 'turn';
        $(`.${relative} .control`).textContent = control ? 'control' : '';
        $(`.${relative} .passed`).textContent = '';
        if (millis !== null) {
            this.progressBars[relative].set(millis);
        }
        if (relative === 'my') {
            this.updatePlayable();
        }
    }

    async onWin({ seat }) {
        const relative = this.seatToRelative[seat];
        $(`.${relative} .win`).textContent = 'win';
    }

    async onDisconnected({ seat }) {
        const relative = this.seatToRelative[seat];
        $(`.${relative} .bot`).textContent = 'bot';
    }

    async updateActionTimer() {
        const checked = $('#enable-timer').checked;
        const value = parseInt($('#set-timer').value, 10);
        $('#set-timer').disabled = !checked;
        const millis = checked ? value : null;
        const response = await this.makeRequest('/api/timer', 'PUT', { millis });
        if (!response.ok) {
            $('#error-description').textContent = await response.text();
        }
    }

    async updatePlayable() {
        const cards = [...document.querySelectorAll('.my .cards .card :checked')]
            .map(element => element.parentElement.dataset.card);
        const response = await this.makeRequest('/api/playable', 'POST', { cards });
        const button = $('.my .play');
        button.value = cards.length === 0 ? 'pass' : 'play';
        button.title = await response.text();

        const offTurn = $('.my .turn').innerHTML === ''; 
        button.classList.toggle('off-turn', offTurn);
        button.classList.toggle('unplayable', !response.ok && !offTurn);
    }

}

function chooseRandom(elements, count) {
    const ret = [];
    while (ret.length < count) {
        const index = Math.floor(Math.random(elements.length));
        ret.push(...elements.splice(index, 1));
    }
    return ret;
}

function createBlank() {
    const face = document.createElement('img');
    face.alt = '';
    face.classList.add('card-face');

    const ret = document.createElement('label');
    ret.classList.add('card');
    ret.dataset.card = 'back';
    ret.replaceChildren(face);
    return ret;
}

function createCard(card, game) {
    const check = document.createElement('input');
    check.type = 'checkbox';
    check.classList.add('card-check');
    check.addEventListener('change', game.updatePlayable.bind(game));

    const face = document.createElement('img');
    face.alt = '';
    face.classList.add('card-face');

    const ret = document.createElement('label');
    ret.classList.add('card');
    ret.dataset.card = card;
    ret.replaceChildren(check, face);
    return ret;
}

function throttle(period, callback) {
    let last = 0;
    return async (...args) => {
        const now = Date.now();
        if (now - last < period) return;
        last = now;
        await callback(args);
    };
}

class ProgressBar {
    start = undefined;
    duration = 0;
    id = undefined;

    constructor(wrapper) {
        this.element = document.createElement('progress');
        this.element.max = 100;
        this.element.style.display = 'none';
        wrapper.replaceChildren(this.element);
    }

    set(duration) {
        cancelAnimationFrame(this.id);
        this.start = undefined;
        this.duration = duration;
        this.id = requestAnimationFrame(this.tick.bind(this));
        this.element.style.display = null;
    }

    tick(now) {
        if (this.start === undefined) {
            this.start = now;
        }
        const percent = 100 * (now - this.start) / this.duration;
        this.element.value = percent;
        this.element.textContent = `${percent}%`;
        if (now < this.start + this.duration) {
            this.id = requestAnimationFrame(this.tick.bind(this));
        }
    }

    stop() {
        cancelAnimationFrame(this.id);
        this.element.style.display = 'none';
    }
}

class JsonSeqStream {
    constructor() {
        this.buffer = '';
    }

    transform(chunk, controller) {
        this.buffer += chunk;

        for (;;) {
            let recordSeparator = this.buffer.indexOf('\x1e'); 
            if (recordSeparator === -1) {
                break;
            }
            const item = this.buffer.substring(0, recordSeparator).trim();
            this.buffer = this.buffer.slice(recordSeparator + 1);
            if (item.length > 0) {
                controller.enqueue(JSON.parse(item));
            }
        }

        try {
            controller.enqueue(JSON.parse(this.buffer));
            this.buffer = '';
        } catch {
            // maybe whatever is in the buffer is valid JSON.
            // maybe it's not. we'll finish it later either way
        }
    }

    flush(controller) {
        if (this.buffer.trim().length > 0) {
            controller.enqueue(JSON.parse(this.buffer));
        }
    }
}
